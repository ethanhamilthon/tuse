mod diff;
mod edit;
mod fs;
mod grep;
mod read;
mod write;

use std::path::Path;

pub use fs::resolve;
pub use grep::Opts as GrepOpts;
pub use read::{DEFAULT_LIMIT, Read};

fn fail(s: impl Into<String>) -> String {
    s.into()
}

fn io_fail(path: &Path, e: &std::io::Error) -> String {
    format!("{}: {e}", path.display())
}

pub fn read(
    path: &Path,
    lines: bool,
    offset: usize,
    limit: usize,
    max_image: usize,
) -> Result<Read, String> {
    read::run(path, lines, offset, limit, max_image)
}

/// Writes the file; returns the confirmation and the diff.
pub fn write(path: &Path, content: &str) -> Result<(String, String), String> {
    write::run(path, content)
}

/// Appends text to the file; returns the confirmation and the diff.
pub fn append(path: &Path, text: &str) -> Result<(String, String), String> {
    write::append(path, text)
}

/// Deletes a single file; returns the confirmation and the removal diff.
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
