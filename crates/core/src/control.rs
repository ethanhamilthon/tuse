use crate::event::Event;
use crate::history;
use crate::record::{Record, UserInput};
use crate::session::Session;
use crate::usage::Stats;
use std::sync::atomic::Ordering;
use std::sync::mpsc::{Receiver, Sender, channel};
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

/// Event subscribers. Events before the first `subscribe()` (startup hooks) are kept
/// and handed to the first subscriber.
pub(crate) enum Subs {
    Pending(Vec<Event>),
    Live(Vec<Sender<Event>>),
}

impl Default for Subs {
    fn default() -> Self {
        Subs::Pending(Vec::new())
    }
}

const PENDING_LIMIT: usize = 64;

impl Session {
    pub(crate) fn emit(&self, e: Event) {
        match &mut *self.subs.lock().unwrap() {
            Subs::Pending(buf) => {
                if buf.len() < PENDING_LIMIT {
                    buf.push(e);
                }
            }
            Subs::Live(subs) => subs.retain(|tx| tx.send(e.clone()).is_ok()),
        }
    }

    pub fn subscribe(&self) -> Receiver<Event> {
        let (tx, rx) = channel();
        let mut subs = self.subs.lock().unwrap();
        match &mut *subs {
            Subs::Pending(buf) => {
                for e in buf.drain(..) {
                    let _ = tx.send(e);
                }
                *subs = Subs::Live(vec![tx]);
            }
            Subs::Live(list) => list.push(tx),
        }
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
        self.stop_commands();
        self.stop_hook_runs();
        self.cancel_turn();
    }

    pub(crate) fn cancel_turn(self: &Arc<Self>) {
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
        self.agent_message_hook(partial, "cancelled", &[]);
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

    pub fn set_model(self: &Arc<Self>, model: &str, effort: &str) {
        let mut st = self.state.lock().unwrap();
        let (prev_model, prev_effort) = (st.model.clone(), st.effort.clone());
        let changed = prev_model != model || prev_effort != effort;
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
        if changed {
            self.model_change_hook(&prev_model, &prev_effort, model, effort);
        }
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

    /// Totals of the session, kept up to date as records arrive. Prices come from
    /// SQLite model metadata, so totals are rebuilt if metadata changes.
    pub fn stats(&self) -> Stats {
        let models = crate::db::models();
        let mut st = self.state.lock().unwrap();
        if st.stats.1 != models {
            st.stats = (Stats::of(&st.records, &models), models);
        }
        let mut out = st.stats.0.clone();
        out.context = st.context;
        if st.busy {
            out.calls += st.calls;
        }
        out
    }

    pub fn title(&self) -> String {
        self.state.lock().unwrap().meta.title.clone()
    }

    /// Updates the session name/title and emits `name_change` after the change takes effect.
    pub fn set_name(self: &Arc<Self>, text: &str) -> Result<(), String> {
        let text: String = text
            .trim()
            .replace(['\r', '\n'], " ")
            .chars()
            .take(80)
            .collect();
        if text.is_empty() {
            return Err("empty title".into());
        }
        let mut st = self.state.lock().unwrap();
        let prev = st.meta.title.clone();
        if prev == text {
            return Ok(());
        }
        st.meta.title = text.clone();
        self.save_meta(&mut st);
        drop(st);
        self.name_change_hook((!prev.is_empty()).then_some(prev), text);
        Ok(())
    }
}

/// Sessions without a `Settings` record fall back to their saved metadata.
pub(crate) fn or_default(saved: &str, default: String) -> String {
    if saved.is_empty() {
        default
    } else {
        saved.to_string()
    }
}
