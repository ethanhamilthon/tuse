use crate::types::Config;
use serde_json::Value;
use std::path::{Path, PathBuf};

pub fn home() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default()
}

/// The global config: `$TUSE_GLOBAL_CONFIG`, else `~/.config/tuse/main.lua`.
pub fn global_path() -> PathBuf {
    match std::env::var_os("TUSE_GLOBAL_CONFIG").filter(|v| !v.is_empty()) {
        Some(p) => PathBuf::from(p),
        None => home().join(".config/tuse/main.lua"),
    }
}

/// `--config PATH`, else `./tuse.lua`, else the global config.
pub fn find(explicit: Option<&Path>, cwd: &Path) -> Option<PathBuf> {
    find_in(explicit, cwd, &global_path())
}

pub(crate) fn find_in(explicit: Option<&Path>, cwd: &Path, global: &Path) -> Option<PathBuf> {
    if let Some(p) = explicit {
        return Some(p.to_path_buf());
    }
    [cwd.join("tuse.lua"), global.to_path_buf()]
        .into_iter()
        .find(|p| p.is_file())
}

/// Builds the config from the table a config file returned (as JSON, functions left out).
/// `name` labels the errors.
pub fn from_value(mut value: Value, name: &str) -> Result<Config, String> {
    if value.as_array().is_some_and(Vec::is_empty) || value.is_null() {
        value = Value::Object(Default::default());
    }
    if !value.is_object() {
        return Err(format!("{name}: the config must return a table"));
    }
    crate::themes::resolve(&mut value).map_err(|e| format!("{name}: {e}"))?;
    empty_tables_as_maps(&mut value);
    let config: Config = serde_json::from_value(value).map_err(|e| format!("{name}: {e}"))?;
    config
        .settings
        .ui
        .check()
        .map_err(|e| format!("{name}: settings.ui: {e}"))?;
    crate::functions::validate(&config).map_err(|e| format!("{name}: {e}"))?;
    let theme = config.theme_name();
    if !matches!(theme, "" | "default") && !config.themes.contains_key(theme) {
        return Err(format!("{name}: theme {theme} not found"));
    }
    Ok(config)
}

/// Lua has one kind of table, so `{}` comes as an empty list; maps need it as `{}`.
fn empty_tables_as_maps(v: &mut Value) {
    let Value::Object(m) = v else { return };
    for (k, v) in m.iter_mut() {
        let list = matches!(k.as_str(), "systems" | "models");
        if !list && v.as_array().is_some_and(Vec::is_empty) {
            *v = Value::Object(Default::default());
        }
        if !list {
            empty_tables_as_maps(v);
        }
    }
}

pub fn resolve_path(p: &str, dir: &Path) -> PathBuf {
    if let Some(rest) = p.strip_prefix("~/") {
        return home().join(rest);
    }
    let p = Path::new(p);
    if p.is_absolute() {
        p.to_path_buf()
    } else {
        dir.join(p)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn builds_from_a_table_and_checks_it() {
        let c = from_value(
            json!({"model": "p/a", "settings": {"theme": "b", "ui": {"tool_line": 7}},
                "themes": {"a": {"accent": "red", "info": "green"}, "b": {"extend": "a", "accent": "blue"}},
                "prompts": [], "hooks": {"h": {"events": ["user_message"]}},
                "models": [{"name": "p/a", "window": 1000}]}),
            "t",
        )
        .unwrap();
        let t = c.active_theme();
        assert_eq!((t.accent.as_str(), t.info.as_str()), ("blue", "green"));
        assert_eq!((c.settings.ui.tool_line, c.compact_at("p/a")), (7, 600));
        assert_eq!(c.hooks["h"].events, ["user_message"]);
        assert!(from_value(json!({"settings": {"theme": "nope"}}), "t").is_err());
        let e = from_value(json!({"settings": {"ui": {"input_design": "round"}}}), "t");
        assert!(e.unwrap_err().contains("input_design"));
        assert!(from_value(json!("x"), "t").is_err());
        let s = crate::Settings::default();
        assert_eq!((s.hook_timeout, s.retries), (30, 5));
    }

    #[test]
    fn find_order() {
        let d = std::env::temp_dir().join(format!("tuse-find-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let g = d.join("main.lua");
        assert_eq!(find_in(None, &d, &g), None);
        std::fs::write(&g, "return {}").unwrap();
        assert_eq!(find_in(None, &d, &g), Some(g.clone()));
        std::fs::write(d.join("tuse.lua"), "return {}").unwrap();
        assert_eq!(find_in(None, &d, &g), Some(d.join("tuse.lua")));
        let x = d.join("x.lua");
        assert_eq!(find_in(Some(&x), &d, &g), Some(x));
        std::fs::remove_dir_all(d).ok();
    }
}
