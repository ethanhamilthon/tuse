use crate::Event;
use crate::Session;
use tuse_api::Delta;
impl Session {
    pub(crate) fn on_delta(&self, d: Delta) {
        match d {
            Delta::Tool {
                index,
                id,
                name,
                code,
            } => {
                if name == crate::session::tool::TOOL {
                    self.emit(Event::ToolDelta {
                        index,
                        id: id.into(),
                        name: name.into(),
                        code: code.into(),
                    });
                }
            }
            Delta::Text(t) => {
                self.state.lock().unwrap().partial.push_str(t);
                self.emit(Event::Text(t.to_string()));
            }
            Delta::Reasoning(t) => self.emit(Event::Reasoning(t.to_string())),
        }
    }
}
