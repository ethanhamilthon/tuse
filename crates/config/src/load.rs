use crate::types::Config;
use serde_json::Value;
use std::path::{Path, PathBuf};

pub fn home() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default()
}

/// `--config PATH`, else `./tuse.lua`, else the global config.
pub fn find(explicit: Option<&Path>, cwd: &Path) -> Option<PathBuf> {
    find_in(explicit, cwd, crate::global_path())
}

pub(crate) fn find_in(
    explicit: Option<&Path>,
    cwd: &Path,
    global: Option<PathBuf>,
) -> Option<PathBuf> {
    if let Some(p) = explicit {
        return Some(p.to_path_buf());
    }
    let local = cwd.join("tuse.lua");
    if local.is_file() { Some(local) } else { global }
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
    serde_path_to_error::deserialize(value).map_err(|e| {
        let path = e
            .path()
            .iter()
            .map(|s| match s {
                serde_path_to_error::Segment::Seq { index } => format!("[{}]", index + 1),
                s => format!(".{s}"),
            })
            .collect::<String>();
        let path = path.strip_prefix('.').unwrap_or(&path).to_string();
        let path = if path.is_empty() { ".".into() } else { path };
        let msg = e.inner().to_string();
        let msg = msg.split(" at line ").next().unwrap_or(&msg);
        let msg = match msg.starts_with("unknown field `") {
            true => "unknown key",
            false => msg,
        };
        if path == "." {
            format!("{name}: {msg}")
        } else {
            format!("{name}: {path}: {msg}")
        }
    })
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
            json!({"settings": {"ui": {"tool_line": 7}},
                "theme": {"accent": "blue", "info": "green"},
                "prompts": {}, "systems": {}, "hooks": {"h": {"events": ["user_message"]}},
                "open": ["md"]}),
            "t",
        )
        .unwrap();
        let t = &c.theme;
        assert_eq!((t.accent.as_str(), t.info.as_str()), ("blue", "green"));
        assert_eq!(c.settings.ui.tool_line, 7);
        assert_eq!(c.hooks["h"].events, ["user_message"]);
        assert_eq!(c.open, ["md"]);
        assert!(from_value(json!("x"), "t").is_err());
        assert!(from_value(json!({"unknown_key": 1}), "t").is_err());
        let s = crate::Settings::default();
        assert_eq!(s.retries, 5);
    }

    #[test]
    fn find_order() {
        let d = std::env::temp_dir().join(format!("tuse-find-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let g = d.join("init.lua");
        assert_eq!(find_in(None, &d, None), None);
        assert_eq!(find_in(None, &d, Some(g.clone())), Some(g.clone()));
        std::fs::write(d.join("tuse.lua"), "return {}").unwrap();
        assert_eq!(find_in(None, &d, Some(g.clone())), Some(d.join("tuse.lua")));
        let x = d.join("x.lua");
        assert_eq!(find_in(Some(&x), &d, Some(g)), Some(x));
        std::fs::remove_dir_all(d).ok();
    }
}
