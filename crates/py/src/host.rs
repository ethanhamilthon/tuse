use crate::ctx::Ctx;
use crate::fail::{Args, Fail};
use monty_types::MontyObject;
use std::path::PathBuf;

/// What the interpreter needs from its owner: the functions that touch a
/// session (`ask`, `show`, `task_*`, ...), the working directory and the env.
pub trait Host: Send + Sync {
    /// Names of the host functions besides the built-in `sh`.
    fn functions(&self) -> &'static [&'static str];

    fn call(&self, name: &str, args: &Args, ctx: &Ctx) -> Result<MontyObject, Fail>;

    fn cwd(&self) -> PathBuf;

    /// Environment of `sh` and `os.environ`, on top of the process's own.
    fn env(&self) -> Vec<(String, String)>;

    /// Directories searched first by `import` (the config folder).
    fn module_dirs(&self) -> Vec<PathBuf>;
}
