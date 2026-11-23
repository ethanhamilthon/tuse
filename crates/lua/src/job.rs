use crate::sched::Sched;
use mlua::{Lua, MultiValue, Thread, Value};
use std::io::Write;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

/// What ends a job early: a cancel flag set from any thread, and a deadline.
#[derive(Clone, Default)]
pub struct Stop {
    pub cancel: Arc<AtomicBool>,
    pub deadline: Option<Instant>,
}

impl Stop {
    pub fn new(cancel: Arc<AtomicBool>, deadline: Option<Instant>) -> Self {
        Self { cancel, deadline }
    }

    /// `Some` once the job must end: `cancelled` or `timed out`.
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

/// Where `print` of a job goes.
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

/// How a job ended.
pub struct Done {
    /// False after an error, a timeout or a cancel.
    pub ok: bool,
    /// What it printed, then its value or its error.
    pub text: String,
    /// `cancelled` or `timed out`.
    pub stop: Option<&'static str>,
    /// Files changed by the job: (path, diff).
    pub diffs: Vec<(String, String)>,
    /// `data:` URLs the model should see.
    pub images: Vec<String>,
}

/// An external host wait that suspends only the calling coroutine.
pub struct ExtWait {
    pub rx: std::sync::mpsc::Receiver<Result<serde_json::Value, String>>,
    pub pause_timeout: bool,
    pub on_cancel: Option<Box<dyn FnOnce() + Send>>,
}

pub(crate) enum Wait {
    /// Resume with these values.
    Ready(MultiValue),
    Sleep(Instant),
    Sh(u64),
    Ext {
        key: u64,
        paused: Option<Duration>,
    },
}

pub type OnDone = Box<dyn FnOnce(&Lua, Done)>;

/// One coroutine of the scheduler: an agent call, a hook run or a task.
pub struct Job {
    pub stop: Stop,
    pub out: Out,
    pub diffs: Vec<(String, String)>,
    pub images: Vec<String>,
    pub is_hook: bool,
    pub(crate) thread: Thread,
    pub(crate) wait: Wait,
    pub(crate) done: Option<OnDone>,
}

impl Job {
    /// A job that calls `f(args)`; `done` gets its result on the VM thread.
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
            thread: lua.create_thread(f)?,
            wait: Wait::Ready(args),
            done: Some(Box::new(done)),
        })
    }

    /// Queues the job; it starts on the next turn of the scheduler.
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

/// The values a job returned, as the agent reads them (`tuse.render` in the prelude).
fn render(lua: &Lua, vals: MultiValue) -> String {
    if vals.iter().all(Value::is_nil) {
        return String::new();
    }
    let f: Option<mlua::Function> = lua.named_registry_value("tuse.render").ok();
    f.and_then(|f| f.call::<String>(vals).ok())
        .unwrap_or_default()
}

/// Runs `f` on the job that is running now (the caller of a host function).
pub fn with_run<R>(lua: &Lua, f: impl FnOnce(&mut Job) -> R) -> Option<R> {
    let mut s = lua.app_data_mut::<Sched>()?;
    s.current_job().map(f)
}

/// Why the running job must end, if it must.
pub fn stopped(lua: &Lua) -> Option<&'static str> {
    lua.app_data_ref::<Sched>()?.current_stop()?.reason()
}

/// Whether the running job is a hook run.
pub fn is_hook(lua: &Lua) -> bool {
    with_run(lua, |j| j.is_hook).unwrap_or(false)
}

/// Registers an external wait on the scheduler; returns the key to yield on.
pub fn add_wait(lua: &Lua, w: ExtWait) -> mlua::Result<u64> {
    let mut s = lua
        .app_data_mut::<Sched>()
        .ok_or_else(|| mlua::Error::runtime("scheduler is gone"))?;
    Ok(s.add_wait(w))
}
