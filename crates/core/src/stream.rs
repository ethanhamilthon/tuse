use crate::{Event, Session};
use tuse_api::Delta;
impl Session {
    pub(crate) fn on_delta(&self, d: Delta) {
        match d {
            Delta::Tool {
                index,
                id,
                name,
                code,
                title,
            } => {
                if name == crate::tool::TOOL {
                    if let Some(t) = title {
                        self.set_status(t.into());
                    }
                    self.emit(Event::ToolDelta {
                        index,
                        id: id.into(),
                        name: name.into(),
                        code: code.into(),
                        title: title.map(str::to_string),
                    });
                }
            }
            Delta::Text(t) => self.emit(Event::Text(t.to_string())),
            Delta::Reasoning(t) => self.emit(Event::Reasoning(t.to_string())),
        }
    }
}
