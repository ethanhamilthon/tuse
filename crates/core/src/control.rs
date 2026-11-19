use crate::event::Event;
use crate::history;
use crate::record::{Record, UserInput};
use crate::session::Session;
use crate::usage::{self, Stats};
use std::sync::atomic::Ordering;
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, atomic::AtomicBool};
use std::time::{Duration, Instant};

impl Session {
    pub fn subscribe(&self) -> Receiver<Event> {
        let (tx, rx) = channel();
        self.subs.lock().unwrap().push(tx);
        rx
    }

    /// Queues a message; it starts a turn when idle, or joins the running turn
    /// after its current step.
    pub fn submit(self: &Arc<Self>, input: UserInput) {
        let mut st = self.state.lock().unwrap();
        st.queue.push(input);
        if st.busy || self.dir.lock().unwrap().is_none() {
            return;
        }
        st.busy = true;
        st.generation += 1;
        st.cancel = Arc::new(AtomicBool::new(false));
        st.turn_start = Instant::now();
        st.calls = 0;
        let (generation, cancel) = (st.generation, st.cancel.clone());
        drop(st);
        let s = self.clone();
        std::thread::spawn(move || s.turn(generation, cancel));
    }

    /// Stops the running turn right away; its thread finds itself stale and exits.
    pub fn cancel(&self) {
        let mut st = self.state.lock().unwrap();
        if !st.busy {
            return;
        }
        st.cancel.store(true, Ordering::Relaxed);
        st.generation += 1;
        st.busy = false;
        st.queue.clear();
        for r in history::missing_results(&st.records) {
            self.push(&mut st, r);
        }
        let elapsed = st.turn_start.elapsed();
        let calls = st.calls;
        self.push(
            &mut st,
            Record::TurnEnd {
                elapsed_ms: elapsed.as_millis() as u64,
                calls,
                cancelled: true,
            },
        );
        self.save_meta(&mut st);
        drop(st);
        self.emit(Event::TurnEnd {
            elapsed,
            calls,
            cancelled: true,
        });
        self.hook("turn_end", vec![("TUSE_CANCELLED".into(), "1".into())]);
    }

    pub fn records(&self) -> Vec<Record> {
        self.state.lock().unwrap().records.clone()
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

    /// Elapsed time and tool calls of the running turn.
    pub fn turn_info(&self) -> Option<(Duration, usize)> {
        let st = self.state.lock().unwrap();
        st.busy.then(|| (st.turn_start.elapsed(), st.calls))
    }

    pub fn stats(&self) -> Stats {
        let config = self.config();
        let st = self.state.lock().unwrap();
        let mut out = Stats {
            context: st.context,
            ..Default::default()
        };
        for r in &st.records {
            match r {
                Record::Usage { model, usage: u } => {
                    out.totals.add(u);
                    out.requests += 1;
                    if let Some(p) = config.model_def(model).and_then(|d| d.prices.as_ref()) {
                        out.cost += usage::cost(u, p);
                        out.priced = true;
                    }
                }
                Record::TurnEnd { calls, .. } => {
                    out.turns += 1;
                    out.calls += calls;
                }
                _ => {}
            }
        }
        if st.busy {
            out.calls += st.calls;
        }
        out
    }

    pub fn usage_report(&self) -> String {
        let (model, effort) = self.model();
        usage::report(&self.id, &model, &effort, &self.config(), &self.stats())
    }

    /// Shows a choice in the TUI; the receiver gets the answer.
    pub fn ask(
        &self,
        question: &str,
        options: Vec<String>,
    ) -> Result<(String, Receiver<String>), String> {
        if self.headless {
            return Err("no TUI is attached to this session (headless)".into());
        }
        let id = format!("a{}", self.ask_seq.fetch_add(1, Ordering::Relaxed) + 1);
        let (tx, rx) = channel();
        self.asks.lock().unwrap().insert(id.clone(), tx);
        self.emit(Event::Ask {
            id: id.clone(),
            question: question.to_string(),
            options,
        });
        Ok((id, rx))
    }

    /// Answers an ask; `None` dismisses it.
    pub fn answer(&self, id: &str, answer: Option<String>) {
        if let Some(tx) = self.asks.lock().unwrap().remove(id)
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
