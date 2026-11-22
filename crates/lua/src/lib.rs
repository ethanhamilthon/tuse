//! The Lua 5.4 runtime of a session: one VM on its own thread and a scheduler of
//! coroutines (the agent's calls, hooks and tasks). `sh` and `sleep` yield, so the
//! other coroutines keep running while one waits.

mod api;
mod job;
mod sched;
mod sh;
mod vm;

pub use api::{to_lua, wrap};
pub use job::{Done, Job, Out, Stop, stopped, with_run};
pub use mlua;
pub use sh::kill_group;
pub use vm::Vm;

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

/// A task started by `spawn`: its ID, its log and the flag that stops it.
pub struct TaskStart {
    pub id: String,
    pub log: PathBuf,
    pub cancel: Arc<AtomicBool>,
}

/// What the VM needs from its owner.
pub trait Host: Send + Sync + 'static {
    /// The working directory of `sh`.
    fn cwd(&self) -> PathBuf;
    /// Environment of `sh`, on top of the process's own.
    fn env(&self) -> Vec<(String, String)>;
    fn task_start(&self, name: &str) -> Result<TaskStart, String>;
    /// Runs on the VM thread when a task ends.
    fn task_end(&self, id: &str, done: Done);
}

/// A Lua error as the agent reads it: the message, then the traceback.
pub fn message(e: &mlua::Error) -> String {
    match e {
        mlua::Error::CallbackError { traceback, cause } => {
            format!("{}\n{traceback}", message(cause))
        }
        mlua::Error::RuntimeError(s) => s.clone(),
        mlua::Error::SyntaxError { message, .. } => message.clone(),
        mlua::Error::WithContext { context, cause } => format!("{context}: {}", message(cause)),
        other => other.to_string(),
    }
}
