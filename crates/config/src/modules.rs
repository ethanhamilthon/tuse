use std::path::{Path, PathBuf};

pub fn tuse_home() -> PathBuf {
    std::env::home_dir().unwrap_or_default().join(".tuse")
}
pub fn plugins_dir() -> PathBuf {
    tuse_home().join("plugins")
}
pub fn plugins_meta_path() -> PathBuf {
    tuse_home().join("plugins.json")
}
pub fn resolve_path(p: &str, dir: &Path) -> PathBuf {
    let home = || std::env::home_dir().unwrap_or_default();
    if p == "~" {
        return home();
    }
    if let Some(rest) = p.strip_prefix("~/") {
        return home().join(rest);
    }
    let path = Path::new(p);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        dir.join(path)
    }
}
pub fn find_primary(explicit: Option<&Path>, cwd: &Path) -> Option<PathBuf> {
    explicit.map(Path::to_path_buf).or_else(|| {
        let local = cwd.join("tuse.lua");
        local.is_file().then_some(local)
    })
}
pub fn global_config_override(cwd: &Path) -> Option<PathBuf> {
    let raw = std::env::var("TUSE_GLOBAL_CONFIG").ok()?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    let resolved = resolve_path(trimmed, cwd);
    Some(if resolved.is_dir() {
        resolved.join("init.lua")
    } else {
        resolved
    })
}
pub fn global_target(cwd: &Path) -> Result<Option<PathBuf>, String> {
    if let Some(p) = global_config_override(cwd) {
        return if p.is_file() {
            Ok(Some(p))
        } else {
            Err(format!(
                "TUSE_GLOBAL_CONFIG: {}: file not found",
                p.display()
            ))
        };
    }
    Ok(default_global_candidates()
        .into_iter()
        .find(|p| p.is_file()))
}
pub fn module_candidates(name: &str, base: &Path) -> Vec<PathBuf> {
    if name == "global" {
        return match global_config_override(base) {
            Some(p) => vec![p],
            None => default_global_candidates(),
        };
    }
    if name.ends_with(".lua") || name.starts_with("~/") || name.starts_with('/') {
        return lua_files(resolve_path(name, base));
    }
    if name.starts_with("./") || name.starts_with("../") {
        return lua_files(base.join(name));
    }
    lua_files(base.join(name.replace('.', "/")))
}
fn default_global_candidates() -> Vec<PathBuf> {
    let d = std::env::home_dir().unwrap_or_default().join(".config");
    vec![d.join("tuse.lua"), d.join("tuse/init.lua")]
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
