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
    /// `agent_message`: running background tasks, `(id, name, kind)`.
    pub tasks: Option<Vec<[String; 3]>>,
    /// `agent_message`: running descendant sessions.
    pub children: Option<Vec<crate::children::RunningChild>>,
}

fn entries(rows: Vec<Vec<(&str, MontyObject)>>) -> MontyObject {
    MontyObject::list(
        rows.into_iter()
            .map(|r| MontyObject::dict(r.into_iter().map(|(k, v)| (MontyObject::string(k), v)))),
    )
}

fn opt(s: Option<String>) -> MontyObject {
    s.map_or_else(MontyObject::none, MontyObject::string)
}

fn listens(hook: &tuse_config::Hook, mode: tuse_config::Mode, event: &str) -> bool {
    !hook.run.trim().is_empty() && hook.matches(mode) && hook.events.iter().any(|e| e == event)
}

impl Session {
    pub(crate) fn hook(self: &Arc<Self>, event: &str, data: Data) {
        let model = self.model().0;
        let mut queues = self.hooks.lock().unwrap();
        let Some(queues) = queues.as_mut() else {
            return;
        };
        let config = self.config();
        let mode = self.mode.into();
        for (name, hook) in &config.hooks {
            if !listens(hook, mode, event) {
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
                (
                    "tasks".into(),
                    data.tasks.clone().map_or_else(MontyObject::none, |t| {
                        entries(
                            t.into_iter()
                                .map(|[id, name, kind]| {
                                    vec![
                                        ("id", MontyObject::string(id)),
                                        ("name", MontyObject::string(name)),
                                        ("kind", MontyObject::string(kind)),
                                    ]
                                })
                                .collect(),
                        )
                    }),
                ),
                (
                    "children".into(),
                    data.children.clone().map_or_else(MontyObject::none, |c| {
                        entries(
                            c.into_iter()
                                .map(|c| {
                                    vec![
                                        ("id", MontyObject::string(c.id)),
                                        ("name", MontyObject::string(c.name)),
                                        ("cwd", MontyObject::string(c.cwd)),
                                        ("elapsed", MontyObject::int(c.elapsed_secs as i64)),
                                    ]
                                })
                                .collect(),
                        )
                    }),
                ),
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
        let listened = self
            .config()
            .hooks
            .values()
            .any(|h| listens(h, self.mode.into(), "agent_message"));
        if !listened {
            return;
        }
        let tasks = (self.tasks.list().into_iter())
            .filter(|t| t.status == "running")
            .map(|t| [t.id, t.name, t.kind])
            .collect();
        let data = Data {
            text: Some(text),
            status: Some(status),
            tasks: Some(tasks),
            children: Some(crate::children::running_descendants(&self.id)),
            ..Default::default()
        };
        self.hook("agent_message", data);
    }
}
