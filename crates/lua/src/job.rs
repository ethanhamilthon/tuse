use crate::sched::Sched;
use mlua::{Lua, MultiValue, Thread, Value};
use std::io::Write;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

#[derive(Clone, Default)]
pub struct Stop {
    pub cancel: Arc<AtomicBool>,
    pub deadline: Option<Instant>,
}

impl Stop {
    pub fn new(cancel: Arc<AtomicBool>, deadline: Option<Instant>) -> Self {
        Self { cancel, deadline }
    }

    pub fn reason(&self) -> Option<&'static str> {
        if self.cancel.load(Ordering::Relaxed) {
            Some("cancelled")
        } else if self.deadline.is_some_and(|d| Instant::now() >= d) {
            Some("timed out")
        } else {
            None
        }
    }
}

pub enum Out {
    Buf(String),
    File(std::fs::File),
}

impl Out {
    pub fn put(&mut self, s: &str) {
        match self {
            Out::Buf(b) => b.push_str(s),
            Out::File(f) => {
                let _ = f.write_all(s.as_bytes());
            }
        }
    }
}

pub struct Done {
    pub ok: bool,
    pub text: String,
    pub stop: Option<&'static str>,
    pub diffs: Vec<(String, String)>,
    pub images: Vec<String>,
}

pub struct ExtWait {
    pub rx: std::sync::mpsc::Receiver<Result<serde_json::Value, String>>,
    pub pause_timeout: bool,
    pub on_cancel: Option<Box<dyn FnOnce() + Send>>,
}

pub(crate) enum Wait {
    Ready(MultiValue),
    Sleep(Instant),
    Sh(u64),
    Ext { key: u64, paused: Option<Duration> },
}

pub type OnDone = Box<dyn FnOnce(&Lua, Done)>;

pub struct Job {
    pub stop: Stop,
    pub out: Out,
    pub diffs: Vec<(String, String)>,
    pub images: Vec<String>,
    pub is_hook: bool,
    pub is_lua_hook: bool,
    pub is_agent: bool,
    pub(crate) thread: Thread,
    pub(crate) wait: Wait,
    pub(crate) done: Option<OnDone>,
}

impl Job {
    pub fn new(
        lua: &Lua,
        f: mlua::Function,
        args: MultiValue,
        stop: Stop,
        out: Out,
        done: impl FnOnce(&Lua, Done) + 'static,
    ) -> mlua::Result<Self> {
        Ok(Self {
            stop,
            out,
            diffs: vec![],
            images: vec![],
            is_hook: false,
            is_lua_hook: false,
            is_agent: false,
            thread: lua.create_thread(f)?,
            wait: Wait::Ready(args),
            done: Some(Box::new(done)),
        })
    }

    pub fn start(self, lua: &Lua) {
        if let Some(mut s) = lua.app_data_mut::<Sched>() {
            s.add(self);
        }
    }

    pub(crate) fn finish(mut self, lua: &Lua, r: Result<MultiValue, String>) {
        let stop = self.stop.reason();
        let ok = r.is_ok();
        let tail = match r {
            Ok(vals) => render(lua, vals),
            Err(e) => match stop {
                Some(why) if e.contains(why) => format!("[{why}]"),
                _ => e,
            },
        };
        let mut text = match std::mem::replace(&mut self.out, Out::Buf(String::new())) {
            Out::Buf(b) => b,
            Out::File(mut f) => {
                if !tail.is_empty() {
                    let _ = writeln!(f, "{tail}");
                }
                String::new()
            }
        };
        if !tail.is_empty() {
            if !text.is_empty() && !text.ends_with('\n') {
                text.push('\n');
            }
            text.push_str(&tail);
        }
        let done = Done {
            ok: ok && stop.is_none(),
            text,
            stop: if ok { None } else { stop },
            diffs: std::mem::take(&mut self.diffs),
            images: std::mem::take(&mut self.images),
        };
        if let Some(f) = self.done.take() {
            f(lua, done);
        }
    }
}

fn render(lua: &Lua, vals: MultiValue) -> String {
    if vals.iter().all(Value::is_nil) {
        return String::new();
    }
    let f: Option<mlua::Function> = lua.named_registry_value("tuse.render").ok();
    f.and_then(|f| f.call::<String>(vals).ok())
        .unwrap_or_default()
}

pub fn with_run<R>(lua: &Lua, f: impl FnOnce(&mut Job) -> R) -> Option<R> {
    let mut s = lua.app_data_mut::<Sched>()?;
    s.current_job().map(f)
}

pub fn stopped(lua: &Lua) -> Option<&'static str> {
    lua.app_data_ref::<Sched>()?.current_stop()?.reason()
}

pub fn is_hook(lua: &Lua) -> bool {
    with_run(lua, |j| j.is_hook).unwrap_or(false)
}

pub fn is_lua_hook(lua: &Lua) -> bool {
    with_run(lua, |j| j.is_lua_hook).unwrap_or(false)
}

pub fn add_wait(lua: &Lua, w: ExtWait) -> mlua::Result<u64> {
    let mut s = lua
        .app_data_mut::<Sched>()
        .ok_or_else(|| mlua::Error::runtime("scheduler is gone"))?;
    Ok(s.add_wait(w))
}
