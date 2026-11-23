use crate::load::{home, resolve_path};
use std::path::{Path, PathBuf};

/// The files `require(name)` tries, in order, relative to `base`:
/// - `global`: the global config (see [`global_candidates`]);
/// - `~/path` or `/path`: `path.lua`, then `path/init.lua`;
/// - a name ending in `.lua`: that file (`~/`, absolute or relative to `base`);
/// - `name` or `a.b`: `base/name.lua`, then `base/name/init.lua` (dots are folders).
pub fn module_candidates(name: &str, base: &Path) -> Vec<PathBuf> {
    if name == "global" {
        return global_candidates();
    }
    if name.ends_with(".lua") || name.starts_with("~/") || name.starts_with('/') {
        return lua_files(resolve_path(name, base));
    }
    if name.starts_with("./") || name.starts_with("../") {
        return lua_files(base.join(name));
    }
    lua_files(base.join(name.replace('.', "/")))
}

/// `$TUSE_GLOBAL_CONFIG` (a file, or a module path like `~/x`), else
/// `~/.config/tuse.lua`, else `~/.config/tuse/init.lua`.
pub fn global_candidates() -> Vec<PathBuf> {
    match std::env::var("TUSE_GLOBAL_CONFIG")
        .ok()
        .filter(|v| !v.is_empty())
    {
        Some(v) => {
            let p = resolve_path(&v, &home());
            if p.is_file() { vec![p] } else { lua_files(p) }
        }
        None => {
            let d = home().join(".config");
            vec![d.join("tuse.lua"), d.join("tuse/init.lua")]
        }
    }
}

/// The global config file, when one exists.
pub fn global_path() -> Option<PathBuf> {
    global_candidates().into_iter().find(|p| p.is_file())
}

fn lua_files(base: PathBuf) -> Vec<PathBuf> {
    if base.extension().is_some_and(|e| e == "lua") {
        return vec![base];
    }
    let mut file = base.clone().into_os_string();
    file.push(".lua");
    vec![PathBuf::from(file), base.join("init.lua")]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn module_names_map_to_files() {
        let cwd = Path::new("/p");
        let h = home();
        let c = |n| module_candidates(n, cwd);
        assert_eq!(c("name"), [cwd.join("name.lua"), cwd.join("name/init.lua")]);
        assert_eq!(c("a.b"), [cwd.join("a/b.lua"), cwd.join("a/b/init.lua")]);
        assert_eq!(c("x.lua"), [cwd.join("x.lua")]);
        assert_eq!(c("./x"), [cwd.join("./x.lua"), cwd.join("./x/init.lua")]);
        assert_eq!(c("../x"), [cwd.join("../x.lua"), cwd.join("../x/init.lua")]);
        assert_eq!(c("~/m"), [h.join("m.lua"), h.join("m/init.lua")]);
        assert_eq!(c("~/m/file.lua"), [h.join("m/file.lua")]);
        assert_eq!(
            c("/abs/m"),
            [
                PathBuf::from("/abs/m.lua"),
                PathBuf::from("/abs/m/init.lua")
            ]
        );
    }
}
