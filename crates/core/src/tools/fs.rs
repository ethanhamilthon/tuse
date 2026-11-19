use std::io::Write;
use std::path::{Path, PathBuf};

/// Absolute paths stay, `~/` is the home folder, the rest is relative to `cwd`.
pub fn resolve(p: &str, cwd: &Path) -> PathBuf {
    tuse_config::resolve_path(p, cwd)
}

/// Writes `data` next to `path` and renames it over, so a crash never leaves a
/// half-written file. Keeps the permissions of an existing file and follows a symlink.
pub fn write_atomic(path: &Path, data: &[u8]) -> std::io::Result<()> {
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
