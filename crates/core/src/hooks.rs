use crate::hook_queue::HookQueue;
use crate::session::Session;
use std::sync::Arc;
use tuse_py::MontyObject;

pub use tuse_config::HOOK_EVENTS as EVENTS;

/// The variables of one event; each event sets its own, the rest are `None`.
#[derive(Default)]
pub struct Data {
    /// `user_message` and `agent_message`.
    pub text: Option<String>,
    /// `user_message`: the first message of the session.
    pub is_first: Option<bool>,
    /// `agent_message`: `working`, `finished` or `cancelled`.
    pub status: Option<&'static str>,
    /// `process_start` / `process_end`: `launch`, `new`, `resume`, `reload` or `exit`.
    pub reason: Option<&'static str>,
}

fn opt(s: Option<String>) -> MontyObject {
    s.map_or_else(MontyObject::none, MontyObject::string)
}

impl Session {
    pub(crate) fn hook(self: &Arc<Self>, event: &str, data: Data) {
        let model = self.model().0;
        let mut queues = self.hooks.lock().unwrap();
        let Some(queues) = queues.as_mut() else {
            return;
        };
        let config = self.config();
        for (name, hook) in &config.hooks {
            if hook.run.trim().is_empty() || !hook.events.iter().any(|e| e == event) {
                continue;
            }
            let config_path = tuse_config::find(self.config_arg.as_deref(), &self.cwd)
                .map(|p| p.display().to_string());
            let vars = vec![
                ("event".to_string(), MontyObject::string(event)),
                ("hook".into(), MontyObject::string(name)),
                ("session".into(), MontyObject::string(&self.id)),
                (
                    "cwd".into(),
                    MontyObject::string(self.cwd.display().to_string()),
                ),
                ("model".into(), MontyObject::string(&model)),
                ("config".into(), opt(config_path)),
                ("text".into(), opt(data.text.clone())),
                (
                    "is_first".into(),
                    data.is_first
                        .map_or_else(MontyObject::none, MontyObject::bool),
                ),
                ("status".into(), opt(data.status.map(String::from))),
                ("reason".into(), opt(data.reason.map(String::from))),
            ];
            let queue = queues.entry(name.clone()).or_insert_with(|| {
                HookQueue::new(
                    Arc::downgrade(self),
                    config.clone(),
                    name.clone(),
                    hook.run.clone(),
                )
            });
            let _ = queue.tx.send(vars);
        }
    }

    pub(crate) fn user_message_hook(self: &Arc<Self>, text: String, is_first: bool) {
        let data = Data {
            text: Some(text),
            is_first: Some(is_first),
            ..Default::default()
        };
        self.hook("user_message", data);
    }

    pub(crate) fn agent_message_hook(self: &Arc<Self>, text: String, status: &'static str) {
        let data = Data {
            text: Some(text),
            status: Some(status),
            ..Default::default()
        };
        self.hook("agent_message", data);
    }
}
