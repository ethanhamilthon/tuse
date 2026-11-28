use crate::session::Session;
use serde_json::json;
use std::sync::Arc;

impl Session {
    pub(crate) fn simple_hook(self: &Arc<Self>, event: &'static str) {
        self.hook(event, json!({"event": event}));
    }

    pub(crate) fn lifecycle(self: &Arc<Self>, launch: bool, restored: bool) {
        self.simple_hook("config_load");
        if launch {
            self.simple_hook("process_start");
        }
        self.simple_hook(if restored {
            "session_resumed"
        } else {
            "session_created"
        });
    }

    pub(crate) fn agent_message_hook(self: &Arc<Self>, status: &'static str) {
        self.hook(
            "agent_message",
            json!({"event": "agent_message", "status": status}),
        )
    }
}
