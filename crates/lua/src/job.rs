use crate::sched::Sched;
use mlua::{Lua, MultiValue, Thread};
use std::io::Write;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

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

pub(crate) enum Wait {
    /// Resume with these values.
    Ready(MultiValue),
    Sleep(Instant),
    Sh(u64),
}

pub type OnDone = Box<dyn FnOnce(&Lua, Done)>;

/// One coroutine of the scheduler: an agent call, a hook run or a task.
pub struct Job {
    pub stop: Stop,
    pub out: Out,
    pub diffs: Vec<(String, String)>,
    pub images: Vec<String>,
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
