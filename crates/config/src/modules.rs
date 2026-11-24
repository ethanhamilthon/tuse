use std::path::{Path, PathBuf};

pub fn resolve_path(p: &str, dir: &Path) -> PathBuf {
    if let Some(rest) = p.strip_prefix("~/") {
        return std::env::home_dir().unwrap_or_default().join(rest);
    }
    let p = Path::new(p);
    if p.is_absolute() {
        p.to_path_buf()
    } else {
        dir.join(p)
    }
}

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

fn global_candidates() -> Vec<PathBuf> {
    let d = std::env::home_dir().unwrap_or_default().join(".config");
    vec![d.join("tuse.lua"), d.join("tuse/init.lua")]
}

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
        let h = std::env::home_dir().unwrap_or_default();
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
