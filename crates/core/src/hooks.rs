use crate::hook_queue::HookQueue;
use crate::session::Session;
use std::sync::Arc;
use tuse_py::MontyObject;

pub const EVENTS: [&str; 7] = [
    "session_start",
    "session_end",
    "turn_start",
    "turn_end",
    "tool_start",
    "tool_end",
    "error",
];

/// What a hook may read besides the event itself; unset names are `None`
/// (`cancelled` is `False`).
#[derive(Default)]
pub struct Data {
    pub tool: Option<String>,
    pub input: Option<String>,
    pub title: Option<String>,
    pub output: Option<String>,
    pub code: Option<i32>,
    pub error: Option<String>,
    pub cancelled: bool,
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
            let vars = vec![
                ("event".to_string(), MontyObject::string(event)),
                ("hook".into(), MontyObject::string(name)),
                ("session".into(), MontyObject::string(&self.id)),
                (
                    "cwd".into(),
                    MontyObject::string(self.cwd.display().to_string()),
                ),
                ("model".into(), MontyObject::string(&model)),
                (
                    "config".into(),
                    tuse_config::find(self.config_arg.as_deref(), &self.cwd)
                        .map_or_else(MontyObject::none, |p| {
                            MontyObject::string(p.display().to_string())
                        }),
                ),
                ("tool".into(), opt(data.tool.clone())),
                ("input".into(), opt(data.input.clone())),
                ("title".into(), opt(data.title.clone())),
                ("output".into(), opt(data.output.clone())),
                (
                    "code".into(),
                    data.code
                        .map_or_else(MontyObject::none, |c| MontyObject::int(c.into())),
                ),
                ("error".into(), opt(data.error.clone())),
                ("cancelled".into(), MontyObject::bool(data.cancelled)),
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
}
