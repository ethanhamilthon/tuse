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
    let config: Config = serde_path_to_error::deserialize(value).map_err(|e| {
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
        let msg = if msg.starts_with("unknown field `") {
            "unknown key"
        } else if path == "providers" || path.starts_with("providers.") {
            "invalid provider settings"
        } else {
            msg
        };
        if path == "." {
            format!("{name}: {msg}")
        } else {
            format!("{name}: {path}: {msg}")
        }
    })?;
    config
        .ui
        .theme
        .check()
        .map_err(|m| format!("{name}: {m}"))?;
    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn builds_from_a_table_and_checks_it() {
        let c = from_value(
            json!({"ui": {"lines": {"tool": 7}, "theme": {"accent": "blue", "markdown": {"heading": "green"}}},
                "history": {"hide": ["hook"]}, "open": ["md"],
                "prompts": {"a": "x", "b": {"kind": "function", "args": ["p"]}},
                "systems": {}, "hooks": {"h": {"events": ["user_message"]}}}),
            "t",
        )
        .unwrap();
        let t = &c.ui.theme;
        assert_eq!(
            (t.accent.as_str(), t.markdown.heading.as_str()),
            ("blue", "green")
        );
        assert_eq!(c.ui.lines.tool, 7);
        assert_eq!(c.history.hide, ["hook"]);
        assert_eq!(c.hooks["h"].events, ["user_message"]);
        assert_eq!(c.open, ["md"]);
        assert_eq!(c.prompts["a"].body, "x");
        assert_eq!(c.prompts["b"].args, ["p"]);
        assert!(from_value(json!({"history": {"hide": ["hooks"]}}), "t").is_err());
        assert!(from_value(json!("x"), "t").is_err());
        assert!(from_value(json!({"unknown_key": 1}), "t").is_err());
    }

    #[test]
    fn ui_defaults_ranges_and_old_keys() {
        let cfg = |v| from_value(v, "test");
        let c = cfg(json!({})).unwrap();
        let (l, s) = (&c.ui.lines, &c.ui.space);
        assert_eq!((l.strings, l.input, l.list, l.diff), (1, 10, 16, 12));
        assert_eq!((s.padding, s.steps, s.messages), (0, 1, 1));
        assert_eq!(c.ui.input, crate::InputStyle::Boxed);
        for bad in [
            json!({"ui": {"lines": {"input": 0}}}),
            json!({"ui": {"lines": {"list": 0}}}),
            json!({"ui": {"lines": {"diff": -2}}}),
            json!({"ui": {"lines": {"fold_strings": true}}}),
            json!({"ui": {"space": {"pad_x": 1}}}),
            json!({"ui": {"scroll_step": 3}}),
            json!({"ui": {"input": "classic"}}),
            json!({"settings": {}}),
        ] {
            assert!(cfg(bad).is_err());
        }
        let err = cfg(json!({"ui": {"input": "other"}})).unwrap_err();
        assert!(err.contains("ui.input"), "{err}");
    }

    #[test]
    fn input_style_serializes() {
        for given in ["boxed", "fill", "bar", "lines"] {
            let c = from_value(json!({"ui": {"input": given}}), "t").unwrap();
            assert_eq!(serde_json::to_value(c).unwrap()["ui"]["input"], given);
        }
    }

    #[test]
    fn theme_defaults_links_and_errors() {
        let cfg = |v| from_value(v, "test");
        let c = cfg(json!({})).unwrap();
        let t = &c.ui.theme;
        assert_eq!(t.user, "fg");
        assert_eq!(t.resolve(&t.input), t.fg);
        assert_eq!(t.resolve(&t.icons), t.accent);
        assert_eq!(t.code.values().len(), crate::CodeTheme::TOKENS.len());
        let c =
            cfg(json!({"ui": {"theme": {"accent": "#FFB86C", "code": {"operator": "accent"}}}}))
                .unwrap();
        let t = &c.ui.theme;
        assert_eq!(t.resolve(&t.code.operator), "#FFB86C");
        assert_eq!(t.resolve(&t.icons), "#FFB86C");
        for bad in [
            json!({"ui": {"theme": {"code": {"nope": "red"}}}}),
            json!({"ui": {"theme": {"code": {"keyword": "nope"}}}}),
            json!({"ui": {"theme": {"user": "user"}}}),
            json!({"ui": {"theme": {"user": "input", "input": "user"}}}),
            json!({"ui": {"theme": {"surface": "red"}}}),
            json!({"ui": {"theme": {"lua": {}}}}),
        ] {
            assert!(cfg(bad).is_err());
        }
        let err = cfg(json!({"ui": {"theme": {"user": "input", "input": "user"}}})).unwrap_err();
        assert!(err.contains("link cycle"), "{err}");
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
