use crate::record::Record;
use crate::session::{Session, State};

impl Session {
    pub(crate) fn push(&self, st: &mut State, r: Record) {
        if let Some(d) = self.dir.lock().unwrap().as_mut() {
            d.append(&r);
        }
        let config = st.stats.1.clone();
        st.stats.0.add(&r, &config);
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

    /// Environment of `sh` and tasks.
    pub fn env(&self) -> Vec<(String, String)> {
        let model = self.state.lock().unwrap().model.clone();
        let mut env = vec![
            ("TUSE_SESSION".into(), self.id.clone()),
            ("TUSE_MODE".into(), self.mode.name().into()),
            ("TUSE_CWD".into(), self.cwd.display().to_string()),
            ("TUSE_MODEL".into(), model),
        ];
        if let Some(c) = tuse_config::find(self.config_arg.as_deref(), &self.cwd) {
            env.push(("TUSE_CURRENT_CONFIG".into(), c.display().to_string()));
        }
        env
    }
}
