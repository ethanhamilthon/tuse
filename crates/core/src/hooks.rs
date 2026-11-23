use crate::record::Record;
use crate::session::Session;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tuse_lua::mlua::{Function, Lua, MultiValue};
use tuse_lua::{Done, Job, Out, Stop};

pub use tuse_config::HOOK_EVENTS as EVENTS;

/// The variables of one event; each event sets its own, the rest are `nil`.
#[derive(Default)]
pub struct Data {
    pub text: Option<String>,
    pub is_first: Option<bool>,
    pub status: Option<&'static str>,
    pub reason: Option<&'static str>,
    /// `agent_message`: running background tasks, `(id, name)`.
    pub tasks: Option<Vec<[String; 2]>>,
}

/// Per hook: whether a run is going, the events waiting behind it, and the flag
/// that stops the running one.
#[derive(Default)]
pub(crate) struct Lane {
    busy: bool,
    queue: Vec<Value>,
    cancel: Arc<AtomicBool>,
}

pub(crate) type Runs = Arc<Mutex<HashMap<String, Lane>>>;

/// Variables every hook gets; the trace lists only the event's own.
const COMMON: &[&str] = &["event", "hook", "session", "cwd", "model", "config"];

fn listens(hook: &tuse_config::Hook, event: &str) -> bool {
    hook.events.iter().any(|e| e == event)
}

impl Session {
    pub(crate) fn hook(self: &Arc<Self>, event: &str, data: Data) {
        if !self.hooks_on.load(Ordering::Acquire) {
            return;
        }
        let config = self.config();
        let config_path = tuse_config::find(self.config_arg.as_deref(), &self.cwd)
            .map(|p| p.display().to_string());
        for (name, hook) in &config.hooks {
            if !listens(hook, event) {
                continue;
            }
            let vars = json!({
                "event": event,
                "hook": name,
                "session": self.id,
                "cwd": self.cwd.display().to_string(),
                "model": self.model().0,
                "config": config_path,
                "text": data.text,
                "is_first": data.is_first,
                "status": data.status,
                "reason": data.reason,
                "tasks": data.tasks.as_ref().map(|t| t.iter().map(|[id, name]| json!({"id": id, "name": name})).collect::<Vec<_>>())
            });
            let mut runs = self.hook_runs.lock().unwrap();
            let lane = runs.entry(name.clone()).or_default();
            lane.queue.push(vars);
            if !lane.busy {
                lane.busy = true;
                drop(runs);
                self.next_hook(name.clone());
            }
        }
    }

    /// Starts the oldest waiting event of hook `name`, if any.
    fn next_hook(self: &Arc<Self>, name: String) {
        let (vars, cancel) = {
            let mut runs = self.hook_runs.lock().unwrap();
            let Some(lane) = runs.get_mut(&name) else {
                return;
            };
            if lane.queue.is_empty() {
                lane.busy = false;
                return;
            }
            lane.cancel = Arc::default();
            (lane.queue.remove(0), lane.cancel.clone())
        };
        let stop = Stop::new(cancel, None);
        let weak = Arc::downgrade(self);
        let started = Instant::now();
        self.vm.post(move |lua| {
            let s2 = weak.clone();
            let (n2, v2) = (name.clone(), vars.clone());
            let done = move |_: &Lua, d: Done| {
                if let Some(s) = s2.upgrade() {
                    s.hook_done(&n2, &v2, d, started);
                }
            };
            let start = || -> tuse_lua::mlua::Result<()> {
                let f: Function = crate::config_lua::code_of(lua, "hooks", &name)
                    .ok_or_else(|| tuse_lua::mlua::Error::runtime("run is gone"))?;
                let arg = tuse_lua::to_lua(lua, &vars)?;
                let mut job = Job::new(
                    lua,
                    f,
                    MultiValue::from_vec(vec![arg]),
                    stop,
                    Out::Buf(String::new()),
                    done,
                )?;
                job.is_hook = true;
                job.start(lua);
                Ok(())
            };
            if let Err(e) = start()
                && let Some(s) = weak.upgrade()
            {
                let d = Done {
                    ok: false,
                    text: tuse_lua::message(&e),
                    stop: None,
                    diffs: vec![],
                    images: vec![],
                };
                s.hook_done(&name, &vars, d, started);
            }
        });
    }

    fn hook_done(self: &Arc<Self>, name: &str, vars: &Value, d: Done, started: Instant) {
        let error = (!d.ok).then(|| crate::host::error_line(&d.text));
        if let Some(e) = &error {
            self.emit(crate::Event::Notice(format!("hook {name}: {e}")));
        }
        let r = Record::Hook {
            name: name.to_string(),
            event: vars["event"].as_str().unwrap_or("").to_string(),
            vars: describe(vars),
            error,
            ms: started.elapsed().as_millis() as u64,
        };
        let mut st = self.state.lock().unwrap();
        self.push(&mut st, r);
        drop(st);
        self.next_hook(name.to_string());
    }

    /// Fires `process_end` and waits for every hook run to end, with no time limit.
    /// No hook fires after this.
    pub(crate) fn finish_hooks(self: &Arc<Self>, reason: &'static str) {
        self.ui_cancel_hooks(true);
        self.hook(
            "process_end",
            Data {
                reason: Some(reason),
                ..Default::default()
            },
        );
        self.hooks_on.store(false, Ordering::Release);
        while self.hook_runs.lock().unwrap().values().any(|l| l.busy) {
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    /// Esc stops the hook runs going now; later events still fire.
    pub(crate) fn stop_hook_runs(&self) {
        for lane in self.hook_runs.lock().unwrap().values() {
            lane.cancel.store(true, Ordering::Relaxed);
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
        if !self
            .config()
            .hooks
            .values()
            .any(|h| listens(h, "agent_message"))
        {
            return;
        }
        let tasks = (self.tasks.list().into_iter())
            .filter(|t| t.alive())
            .map(|t| [t.id, t.name])
            .collect();
        let data = Data {
            text: Some(text),
            status: Some(status),
            tasks: Some(tasks),
            ..Default::default()
        };
        self.hook("agent_message", data);
    }
}

fn describe(vars: &Value) -> String {
    let Some(m) = vars.as_object() else {
        return String::new();
    };
    m.iter()
        .filter(|(k, v)| !COMMON.contains(&k.as_str()) && !v.is_null())
        .map(|(k, v)| {
            let v = match v {
                Value::String(s) => format!("{s:?}"),
                v => v.to_string(),
            };
            let short: String = v.chars().take(200).collect();
            let cut = if short.len() < v.len() { "…" } else { "" };
            format!("{k}={short}{cut}")
        })
        .collect::<Vec<_>>()
        .join("\n")
}
