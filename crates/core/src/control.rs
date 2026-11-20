use crate::event::Event;
use crate::history;
use crate::hooks::Data;
use crate::record::{Record, UserInput};
use crate::session::Session;
use crate::usage::Stats;
use std::sync::atomic::Ordering;
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, atomic::AtomicBool};
use std::time::{Duration, Instant};

/// Identifies a submission within its session, even when two messages have the same text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InputId(pub(crate) u64);

impl InputId {
    /// The number a client sees (`tuse json`).
    pub fn number(self) -> u64 {
        self.0
    }
    pub fn from_number(n: u64) -> Self {
        InputId(n)
    }
}

impl Session {
    pub fn subscribe(&self) -> Receiver<Event> {
        let (tx, rx) = channel();
        self.subs.lock().unwrap().push(tx);
        rx
    }

    /// Queues a message; it starts a turn when idle, or joins the running turn
    /// after its current step. Returns its ID and whether a turn was already running.
    pub fn submit(self: &Arc<Self>, input: UserInput) -> (InputId, bool) {
        let mut st = self.state.lock().unwrap();
        st.next_input += 1;
        let id = InputId(st.next_input);
        st.queue.push((id, input));
        if st.busy || self.dir.lock().unwrap().is_none() {
            return (id, st.busy);
        }
        st.busy = true;
        st.generation += 1;
        st.cancel = Arc::new(AtomicBool::new(false));
        st.turn_start = Instant::now();
        st.calls = 0;
        st.status = String::new();
        let (generation, cancel) = (st.generation, st.cancel.clone());
        drop(st);
        let s = self.clone();
        std::thread::spawn(move || s.turn(generation, cancel));
        (id, false)
    }

    /// Withdraws just this message, if the turn has not taken it yet.
    pub fn withdraw_input(&self, id: InputId) -> bool {
        let mut st = self.state.lock().unwrap();
        let Some(i) = st.queue.iter().position(|(queued, _)| *queued == id) else {
            return false;
        };
        st.queue.remove(i);
        true
    }

    /// Stops the running turn right away; its thread finds itself stale and exits.
    pub fn cancel(self: &Arc<Self>) {
        let mut st = self.state.lock().unwrap();
        if !st.busy {
            return;
        }
        st.cancel.store(true, Ordering::Relaxed);
        st.generation += 1;
        st.busy = false;
        st.queue.clear();
        let partial = std::mem::take(&mut st.partial);
        for r in history::missing_results(&st.records) {
            self.push(&mut st, r);
        }
        let elapsed = st.turn_start.elapsed();
        let (model, effort) = (st.model.clone(), st.effort.clone());
        let calls = st.calls;
        self.push(
            &mut st,
            Record::TurnEnd {
                elapsed_ms: elapsed.as_millis() as u64,
                calls,
                model: model.clone(),
                effort: effort.clone(),
                cancelled: true,
            },
        );
        self.save_meta(&mut st);
        drop(st);
        self.emit(Event::TurnEnd {
            elapsed,
            calls,
            model,
            effort,
            cancelled: true,
        });
        self.hook(
            "agent_message",
            Data {
                text: Some(partial),
                status: Some("cancelled"),
                ..Default::default()
            },
        );
    }

    pub fn records(&self) -> Vec<Record> {
        self.state.lock().unwrap().records.clone()
    }

    /// Runs `f` on the history without copying it (it can hold large images).
    pub fn with_records<T>(&self, f: impl FnOnce(&[Record]) -> T) -> T {
        f(&self.state.lock().unwrap().records)
    }

    /// `(provider/model, effort)`.
    pub fn model(&self) -> (String, String) {
        let st = self.state.lock().unwrap();
        (st.model.clone(), st.effort.clone())
    }

    pub fn set_model(&self, model: &str, effort: &str) {
        let mut st = self.state.lock().unwrap();
        st.model = model.to_string();
        st.effort = effort.to_string();
        let r = Record::Settings {
            model: model.into(),
            effort: effort.into(),
        };
        self.push(&mut st, r);
        self.save_meta(&mut st);
        drop(st);
        self.emit(Event::Usage);
    }

    pub fn busy(&self) -> bool {
        self.state.lock().unwrap().busy
    }

    /// Elapsed time and the status of the running turn.
    pub fn turn_info(&self) -> Option<(Duration, String)> {
        let st = self.state.lock().unwrap();
        st.busy
            .then(|| (st.turn_start.elapsed(), st.status.clone()))
    }

    /// Totals of the session, kept up to date as records arrive. Prices come from the
    /// config, so the totals are rebuilt once after the config changes.
    pub fn stats(&self) -> Stats {
        let config = self.config();
        let mut st = self.state.lock().unwrap();
        if !Arc::ptr_eq(&st.stats.1, &config) {
            st.stats = (Stats::of(&st.records, &config), config);
        }
        let mut out = st.stats.0.clone();
        out.context = st.context;
        if st.busy {
            out.calls += st.calls;
        }
        out
    }

    /// Shows a choice in the TUI; the receiver gets the answer.
    pub fn ask(
        &self,
        question: &str,
        options: Vec<String>,
    ) -> Result<(String, Receiver<String>), String> {
        self.require_client("ask")?;
        let id = format!("a{}", self.ask_seq.fetch_add(1, Ordering::Relaxed) + 1);
        let (tx, rx) = channel();
        self.state.lock().unwrap().asks.insert(id.clone(), tx);
        self.emit(Event::Ask {
            id: id.clone(),
            question: question.to_string(),
            options,
        });
        Ok((id, rx))
    }

    /// Whether the ask `id` is still waiting for an answer.
    pub fn has_ask(&self, id: &str) -> bool {
        self.state.lock().unwrap().asks.contains_key(id)
    }

    /// Answers an ask; `None` dismisses it.
    pub fn answer(&self, id: &str, answer: Option<String>) {
        let tx = self.state.lock().unwrap().asks.remove(id);
        if let Some(tx) = tx
            && let Some(a) = answer
        {
            let _ = tx.send(a);
        }
        self.emit(Event::AskDone { id: id.to_string() });
    }

    pub fn title(&self) -> String {
        self.state.lock().unwrap().meta.title.clone()
    }
}

/// A new session records the config's model and effort at once and keeps them from then on.
pub(crate) fn initial_settings(config: &tuse_config::Config) -> Record {
    Record::Settings {
        model: config.default_model(),
        effort: config.default_effort(),
    }
}

/// An old session without a `Settings` record keeps what `meta.json` says, if anything.
pub(crate) fn or_default(saved: &str, default: String) -> String {
    if saved.is_empty() {
        default
    } else {
        saved.to_string()
    }
}
