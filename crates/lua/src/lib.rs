mod api;
mod job;
mod sched;
mod sh;
mod vm;

pub use api::{host, make_spawn, to_lua, wrap, yield_wait};
pub use job::{Done, ExtWait, Job, Out, Stop, add_wait, is_hook, is_lua_hook, stopped, with_run};
pub use mlua;
pub use sh::kill_group;
pub use vm::Vm;

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

pub struct TaskStart {
    pub id: String,
    pub log: PathBuf,
    pub cancel: Arc<AtomicBool>,
}

pub trait Host: Send + Sync + 'static {
    fn cwd(&self) -> PathBuf;
    fn action(&self, _what: &str) {}
    fn task_start(&self, name: &str, from_lua_hook: bool) -> Result<TaskStart, String>;
    fn task_end(&self, id: &str, done: Done);
}

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
