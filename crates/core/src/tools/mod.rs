mod diff;
mod edit;
mod grep;
mod read;
mod write;

use std::io::Write;
use std::path::Path;

pub use grep::Opts as GrepOpts;
pub use read::{DEFAULT_LIMIT, Read};

fn fail(s: impl Into<String>) -> String {
    s.into()
}

fn io_fail(path: &Path, e: &std::io::Error) -> String {
    format!("{}: {e}", path.display())
}

fn write_atomic(path: &Path, data: &[u8]) -> std::io::Result<()> {
    let target = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    if let Some(dir) = target.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let name = target
        .file_name()
        .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
    let tmp = target.with_file_name(format!(".{name}.tuse-{}", std::process::id()));
    let result = (|| {
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(data)?;
        if let Ok(meta) = std::fs::metadata(&target) {
            f.set_permissions(meta.permissions())?;
        }
        f.sync_all()?;
        std::fs::rename(&tmp, &target)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result
}

pub fn read(path: &Path, lines: bool, offset: usize, limit: usize) -> Result<Read, String> {
    read::run(path, lines, offset, limit)
}

pub fn write(path: &Path, content: &str) -> Result<(String, String), String> {
    write::run(path, content, false)
}

pub fn append(path: &Path, text: &str) -> Result<(String, String), String> {
    write::run(path, text, true)
}

pub fn delete(path: &Path) -> Result<(String, String), String> {
    write::delete(path)
}

pub fn edit(path: &Path, old: &str, new: &str, all: bool) -> Result<(String, String), String> {
    edit::run(path, old, new, all)
}

pub fn grep(
    pattern: &str,
    o: GrepOpts,
    cwd: &Path,
    stopped: &dyn Fn() -> bool,
) -> Result<String, String> {
    grep::run(pattern, o, cwd, stopped)
}
