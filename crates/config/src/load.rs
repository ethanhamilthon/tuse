use crate::types::Config;
use serde_json::Value;
use std::path::{Path, PathBuf};

pub fn find(explicit: Option<&Path>, cwd: &Path) -> Option<PathBuf> {
    find_in(explicit, cwd, crate::modules::global_path())
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn builds_from_a_table_and_checks_it() {
        let c = from_value(
            json!({"settings": {"ui": {"tool_line": 7, "hide_history": ["hook"], "scope_models": ["a/*"]}, "open": ["md"]},
                "theme": {"accent": "blue", "info": "green"},
                "prompts": {"a": "x", "b": {"kind": "function", "args": ["p"]}},
                "systems": {}, "hooks": {"h": {"events": ["user_message"]}}}),
            "t",
        )
        .unwrap();
        let t = &c.theme;
        assert_eq!((t.accent.as_str(), t.info.as_str()), ("blue", "green"));
        assert_eq!(c.settings.ui.tool_line, 7);
        assert_eq!(c.hooks["h"].events, ["user_message"]);
        assert_eq!(c.settings.open, ["md"]);
        assert_eq!(c.prompts["a"].body, "x");
        assert_eq!(c.prompts["b"].args, ["p"]);
        assert!(crate::scope_matches(&c.settings.ui.scope_models, "a/m"));
        assert!(!crate::scope_matches(&c.settings.ui.scope_models, "b/m"));
        assert!(
            from_value(
                json!({"settings": {"ui": {"hide_history": ["hooks"]}}}),
                "t"
            )
            .is_err()
        );
        assert!(from_value(json!("x"), "t").is_err());
        assert!(from_value(json!({"unknown_key": 1}), "t").is_err());
    }

    #[test]
    fn input_style_defaults_validates_and_serializes() {
        let cfg = |v| from_value(v, "test");
        assert_eq!(
            cfg(json!({})).unwrap().settings.ui.input_style,
            crate::InputStyle::Classic
        );
        let boxed = cfg(json!({"settings":{"ui":{"input_style":"boxed"}}})).unwrap();
        assert_eq!(
            serde_json::to_value(boxed).unwrap()["settings"]["ui"]["input_style"],
            "boxed"
        );
        let err = cfg(json!({"settings":{"ui":{"input_style":"other"}}})).unwrap_err();
        assert!(err.contains("settings.ui.input_style"), "{err}");
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
