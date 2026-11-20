use crate::ctx::Ctx;
use crate::fail::{Args, Fail};
use monty_types::MontyObject;
use std::fs::File;
use std::path::PathBuf;
use std::process::Child;

/// A running process that outlived the timeout of its `sh`.
pub struct Adopt {
    pub child: Child,
    pub cmd: String,
    pub cwd: PathBuf,
}

/// What the interpreter needs from its owner: the functions that touch a
/// session (`ask`, `draw_ui`, `task_*`, ...), the working directory and the env.
pub trait Host: Send + Sync {
    /// Names of the host functions besides the built-in `sh`.
    fn functions(&self) -> &'static [&'static str];

    fn call(&self, name: &str, args: &Args, ctx: &Ctx) -> Result<MontyObject, Fail>;

    fn cwd(&self) -> PathBuf;

    /// Environment of `sh` and `os.environ`, on top of the process's own.
    fn env(&self) -> Vec<(String, String)>;

    /// Directories searched first by `import` (the config folder).
    fn module_dirs(&self) -> Vec<PathBuf>;

    /// Takes over a process whose `sh` timed out: the task id and the file the
    /// rest of its output goes to. `Err` gives the process back to be killed.
    fn adopt(&self, adopt: Adopt) -> Result<(String, File), Adopt> {
        Err(adopt)
    }
}
