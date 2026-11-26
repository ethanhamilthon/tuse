use crate::convo::record::Record;
use crate::convo::usage::Stats;
use crate::event::Event;
use crate::session::bus::Signal;
use crate::session::{Session, State};
use crate::storage::store::{ReadCache, SessionDir};
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;

impl Session {
    pub(crate) fn stop_commands(&self) {
        for c in self.cmd_runs.lock().unwrap().drain(..) {
            c.store(true, Ordering::Relaxed);
        }
    }

    pub(crate) fn cached(&self, st: &mut State) -> Arc<ReadCache> {
        st.cache
            .get_or_insert_with(|| {
                Arc::new(SessionDir::read(&self.id).expect("read session history"))
            })
            .clone()
    }

    pub fn records(&self) -> Vec<Record> {
        self.with_records(<[Record]>::to_vec)
    }

    pub fn with_records<T>(&self, f: impl FnOnce(&[Record]) -> T) -> T {
        let cache = self.cached(&mut self.state.lock().unwrap());
        f(&cache.records)
    }

    pub fn history_tag(&self) -> (i64, i64) {
        self.cached(&mut self.state.lock().unwrap()).tag
    }

    pub fn model(&self) -> (String, String) {
        let cache = self.cached(&mut self.state.lock().unwrap());
        (cache.meta.model.clone(), cache.meta.effort.clone())
    }

    pub fn activity(&self) -> crate::Activity {
        self.state.lock().unwrap().activity
    }

    pub(crate) fn set_activity(&self, st: &mut State, activity: crate::Activity) {
        if st.activity != activity {
            st.activity = activity;
            self.wake_ui();
        }
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
        let mut st = self.state.lock().unwrap();
        let mut out = self.cached(&mut st).stats.clone();
        if st.busy {
            out.calls += st.calls;
        }
        out
    }

    pub fn title(&self) -> String {
        self.cached(&mut self.state.lock().unwrap())
            .meta
            .title
            .clone()
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
        self.emit(Signal::Status(text));
    }
}

impl Session {
    pub(crate) fn write_history(
        &self,
        st: &mut State,
        meta: Option<&crate::storage::store::Meta>,
        replacement: Option<&[Record]>,
        records: &[Record],
        config: &tuse_config::Config,
    ) -> Result<(), String> {
        let dir = self.dir.lock().unwrap();
        dir.as_ref()
            .ok_or("session closed")?
            .write(meta, replacement, records, config)?;
        st.cache = None;
        Ok(())
    }

    pub(crate) fn push(&self, st: &mut State, r: Record) {
        self.record(st, &[r], []);
    }

    pub fn error(&self, text: String) {
        self.message(
            Record::message(crate::Level::Error, text.clone()),
            Event::Error(text),
        );
    }

    pub fn flash(&self, text: String) -> bool {
        self.message(
            Record::message(crate::Level::Flash, text.clone()),
            Event::Flash(text),
        )
    }

    pub(crate) fn message(&self, r: Record, e: Event) -> bool {
        self.record(&mut self.state.lock().unwrap(), &[r], [e])
    }

    pub(crate) fn rebuild_system(
        &self,
        st: &mut State,
        config: &tuse_config::Config,
    ) -> Result<(), String> {
        let text = crate::prompt::system_prompt(config);
        self.dir
            .lock()
            .unwrap()
            .as_ref()
            .ok_or("session closed")?
            .system(&Record::System { text })?;
        st.cache = None;
        Ok(())
    }
}
