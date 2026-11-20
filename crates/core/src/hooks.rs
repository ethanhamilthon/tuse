use crate::event::Event;
use crate::host::{SessionHost, last_line};
use crate::session::Session;
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};
use tuse_py::{Ctx, MontyObject, NEVER, Python};

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
    /// Starts every hook subscribed to `event`, each in its own thread with a
    /// fresh interpreter, and does not wait for them; the handles let `close` do so.
    pub(crate) fn hook(self: &Arc<Self>, event: &str, data: Data) -> Vec<JoinHandle<()>> {
        let config = self.config();
        let mut handles = vec![];
        for (name, hook) in &config.hooks {
            if hook.run.trim().is_empty() || !hook.events.iter().any(|e| e == event) {
                continue;
            }
            let (s, config, name, run) =
                (self.clone(), config.clone(), name.clone(), hook.run.clone());
            let vars = vec![
                ("event".to_string(), MontyObject::string(event)),
                ("hook".into(), MontyObject::string(&name)),
                ("session".into(), MontyObject::string(&self.id)),
                (
                    "cwd".into(),
                    MontyObject::string(self.cwd.display().to_string()),
                ),
                ("model".into(), MontyObject::string(self.model().0)),
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
            handles.push(std::thread::spawn(move || {
                let host = Arc::new(SessionHost::new(Arc::downgrade(&s)));
                let mut py = Python::new(host, config.settings.max_output);
                let secs = config.settings.hook_timeout.max(1);
                let ctx = Ctx::new(&NEVER, Instant::now() + Duration::from_secs(secs));
                let prelude = config.prelude.join("\n");
                if !prelude.trim().is_empty() {
                    py.run(&prelude, &ctx, vec![]);
                }
                let out = py.run(&run, &ctx, vars);
                if !out.ok {
                    s.emit(Event::Notice(format!(
                        "hook {name}: {}",
                        last_line(&out.text)
                    )));
                }
            }));
        }
        handles
    }
}
