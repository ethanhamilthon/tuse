use crate::Session;
use crate::session::bus::Signal;
use tuse_api::Delta;
impl Session {
    pub(crate) fn on_delta(&self, generation: u64, d: Delta) {
        let Some(mut st) = self.lock_live(generation) else {
            return;
        };
        match d {
            Delta::Tool {
                index,
                id,
                name,
                code,
            } => {
                if name == crate::session::turn::tool::TOOL {
                    self.emit(Signal::ToolDelta {
                        index,
                        id: id.into(),
                        name: name.into(),
                        code: code.into(),
                    });
                }
            }
            Delta::Text(t) => {
                st.partial.push_str(t);
                self.emit(Signal::Text(t.to_string()));
            }
            Delta::Reasoning(t) => {
                st.reasoning.push_str(t);
                self.emit(Signal::Reasoning(t.to_string()));
            }
        }
    }
}
