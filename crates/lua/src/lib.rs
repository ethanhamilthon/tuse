mod api;
mod job;
mod sched;
mod sh;
mod value;
mod vm;

pub use api::source_of;
pub use api::{host, wrap, yield_wait};
pub use job::{Done, ExtWait, Job, Out, Stop};
pub use mlua;
pub use sched::{add_wait, is_hook, is_lua_hook, start, stopped, with_run};
pub use sh::{kill_group, setsid};
pub use value::to_lua;
pub use vm::Vm;

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

pub struct TaskStart {
    pub id: String,
    pub log: std::fs::File,
    pub cancel: Arc<AtomicBool>,
}

pub trait Host: Send + Sync + 'static {
    fn cwd(&self) -> PathBuf;
    fn action(&self, _what: &str) {}
    fn task_start(&self, name: &str, code: &str, from_lua_hook: bool) -> Result<TaskStart, String>;
    fn task_track(&self, _id: &str, _name: &str) {}
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
