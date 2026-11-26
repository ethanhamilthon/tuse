use crate::event::Switch;
use crate::session::Session;
use crate::session::bus::Signal;
use std::sync::Arc;

impl Session {
    pub(crate) fn request_switch(self: &Arc<Self>, switch: Switch) -> Result<(), String> {
        let mut st = self.state.lock().unwrap();
        if st.pending_switch.is_some() {
            return Err("a session switch is already pending".into());
        }
        match &switch {
            Switch::Fork(index) => {
                let cache = self.cached(&mut st);
                let records = st.pending_history.as_deref().unwrap_or(&cache.records);
                crate::session::launch::check_fork(records, *index)?;
            }
            Switch::Resume(id) => crate::session::launch::check_resume(self, id)?,
        }
        if st.tool_step {
            st.pending_switch = Some(switch);
        } else {
            drop(st);
            self.emit(Signal::Switch(switch));
        }
        Ok(())
    }
}
