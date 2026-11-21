use crate::record::Record;
use crate::session::{Session, State};
use crate::store::{Meta, SessionDir};

/// Records `TUSE_PARENT` in the meta. A session with a parent creates its folder, lock
/// and `meta.json` now, so `/childrens` of the parent sees it before its first prompt.
pub(crate) fn adopt_parent(dir: &mut SessionDir, meta: &mut Meta) -> Result<(), String> {
    let Some(p) = std::env::var("TUSE_PARENT").ok().filter(|p| !p.is_empty()) else {
        return Ok(());
    };
    meta.parent = p;
    dir.materialize()?;
    dir.write_meta(meta);
    Ok(())
}

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

    /// Environment of `sh`, tasks and `os.environ`.
    pub fn env(&self) -> Vec<(String, String)> {
        let model = self.state.lock().unwrap().model.clone();
        let mut env = vec![
            ("TUSE_SESSION".into(), self.id.clone()),
            ("TUSE_PARENT".into(), self.id.clone()),
            ("TUSE_CWD".into(), self.cwd.display().to_string()),
            ("TUSE_MODEL".into(), model),
        ];
        if let Some(c) = tuse_config::find(self.config_arg.as_deref(), &self.cwd) {
            env.push(("TUSE_CURRENT_CONFIG".into(), c.display().to_string()));
        }
        env
    }
}
