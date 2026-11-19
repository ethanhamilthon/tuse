use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use tuse_config::Config;

/// Writes one shim script per `bins` entry into `dir` (which goes first in the
/// agent's PATH), removing shims of entries that are gone.
pub fn write_shims(config: &Config, dir: &Path, config_dir: &Path) {
    let _ = std::fs::remove_dir_all(dir);
    if config.bins.is_empty() || std::fs::create_dir_all(dir).is_err() {
        return;
    }
    for (name, cmd) in &config.bins {
        if name.is_empty() || name.contains('/') {
            continue;
        }
        let cmd = resolve(cmd, config_dir);
        let path = dir.join(name);
        let script = format!("#!/bin/sh\nexec {cmd} \"$@\"\n");
        if std::fs::write(&path, script).is_ok() {
            let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755));
        }
    }
}

fn resolve(cmd: &str, dir: &Path) -> String {
    let t = cmd.trim();
    let (prog, rest) = t.split_once(' ').unwrap_or((t, ""));
    if prog.starts_with("~/") || (prog.contains('/') && !prog.starts_with('/')) {
        let abs = tuse_config::resolve_path(prog, dir).display().to_string();
        return format!("{abs} {rest}").trim_end().to_string();
    }
    t.to_string()
}
