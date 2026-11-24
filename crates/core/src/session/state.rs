use crate::convo::record::Record;
use crate::convo::usage::Stats;
use crate::event::Event;
use crate::session::{Session, State};
use std::sync::atomic::Ordering;
use std::time::Duration;

impl Session {
    pub(crate) fn stop_commands(&self) {
        for c in self.cmd_runs.lock().unwrap().drain(..) {
            c.store(true, Ordering::Relaxed);
        }
    }

    pub fn records(&self) -> Vec<Record> {
        self.state.lock().unwrap().records.clone()
    }

    pub fn with_records<T>(&self, f: impl FnOnce(&[Record]) -> T) -> T {
        f(&self.state.lock().unwrap().records)
    }

    pub fn model(&self) -> (String, String) {
        let st = self.state.lock().unwrap();
        (st.model.clone(), st.effort.clone())
    }

    pub fn busy(&self) -> bool {
        self.state.lock().unwrap().busy
    }

    pub fn turn_info(&self) -> Option<(Duration, String)> {
        let st = self.state.lock().unwrap();
        st.busy
            .then(|| (st.turn_start.elapsed(), st.status.clone()))
    }

    pub fn stats(&self) -> Stats {
        let st = self.state.lock().unwrap();
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

    pub(crate) fn lock_live(&self, generation: u64) -> Option<std::sync::MutexGuard<'_, State>> {
        let st = self.state.lock().unwrap();
        (st.generation == generation).then_some(st)
    }

    pub(crate) fn set_status(&self, text: String) {
        let mut st = self.state.lock().unwrap();
        if st.status == text {
            return;
        }
        st.status = text.clone();
        drop(st);
        self.emit(Event::Status(text));
    }
}

pub(crate) fn or_default(saved: &str, default: String) -> String {
    if saved.is_empty() {
        default
    } else {
        saved.to_string()
    }
}
