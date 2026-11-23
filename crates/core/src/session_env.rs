use crate::event::Event;
use crate::mode::{Options, Start};
use crate::record::Record;
use crate::session::{Session, State};
use crate::tasks::TaskInfo;
use std::sync::Arc;

impl Session {
    pub fn is_open(&self) -> bool {
        self.dir.lock().unwrap().is_some()
    }

    /// Resolves configured prompt names into `(name, body)` pairs.
    pub fn resolve_prompts(
        &self,
        names: &[String],
        strict: bool,
    ) -> Result<Vec<(String, String)>, String> {
        let config = self.config();
        let mut out = vec![];
        for n in names {
            match config.prompts.get(n) {
                Some(body) => out.push((n.clone(), body.clone())),
                None if strict => return Err(format!("unknown prompt `{n}`")),
                None => {}
            }
        }
        Ok(out)
    }

    /// Stops a background task and notifies front ends.
    pub fn stop_task(&self, id: &str) -> Result<TaskInfo, String> {
        let t = self.tasks.stop(id)?;
        self.emit(Event::Tasks);
        Ok(t)
    }

    /// Prepares a new (`resume: None`) or saved (`resume: Some(id)`) session, then closes this
    /// one and starts it. Any failure (missing or locked session, invalid config) happens
    /// before the close, so this session stays open and keeps its work.
    pub fn switch(self: &Arc<Self>, resume: Option<String>) -> Result<Arc<Session>, String> {
        if resume.as_deref() == Some(self.id.as_str()) {
            return Err(format!("session {} is already open", self.id));
        }
        let start = if resume.is_some() {
            Start::Resume
        } else {
            Start::New
        };
        let next = Session::prepare(Options {
            cwd: self.cwd.clone(),
            config: self.config_arg.clone(),
            mode: self.mode,
            resume,
            start,
        })?;
        self.close_for(start.reason());
        Ok(next.start())
    }

    pub(crate) fn push(&self, st: &mut State, r: Record) {
        if let Some(d) = self.dir.lock().unwrap().as_mut() {
            d.append(&r);
        }
        st.stats.0.add(&r, &st.stats.1);
        st.records.push(r);
    }

    pub(crate) fn save_meta(&self, st: &mut State) {
        st.meta.model = st.model.clone();
        st.meta.effort = st.effort.clone();
        st.meta.updated = crate::id::now();
        if let Some(d) = self.dir.lock().unwrap().as_mut() {
            d.write_meta(&st.meta);
        }
    }
}
