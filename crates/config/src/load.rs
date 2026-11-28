use crate::types::Config;
use serde_json::Value;

pub fn from_value(mut value: Value, name: &str) -> Result<Config, String> {
    if value.as_array().is_some_and(Vec::is_empty) || value.is_null() {
        value = Value::Object(Default::default());
    }
    if !value.is_object() {
        return Err(format!("{name}: the config must return a table"));
    }
    let config: Config = serde_path_to_error::deserialize(value).map_err(|e| {
        let mut path = e
            .path()
            .iter()
            .map(|s| match s {
                serde_path_to_error::Segment::Seq { index } => format!("[{}]", index + 1),
                s => format!(".{s}"),
            })
            .collect::<String>();
        if path.starts_with('.') {
            path.remove(0);
        }
        let raw = e.inner().to_string();
        let msg = raw.split(" at line ").next().unwrap_or(&raw);
        let msg = if msg.starts_with("unknown field `") {
            "unknown key"
        } else {
            msg
        };
        if path.is_empty() {
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
            json!({"ui": {"lines": {"tool": 7}, "picker": "top", "theme": {"accent": "blue", "markdown": {"heading": "green"}}},
                "open": true,
                "prompts": {"a": "x"},
                "systems": {"main": "hi"}, "hook": {"user_message": ["ns"]}}),
            "t",
        )
        .unwrap();
        let t = &c.ui.theme;
        assert_eq!(
            (t.accent.as_str(), t.markdown.heading.as_str()),
            ("blue", "green")
        );
        assert_eq!(c.ui.lines.tool, 7);
        assert_eq!(c.ui.picker, crate::PickerPlacement::Top);
        assert_eq!(c.hook["user_message"], ["ns"]);
        assert!(c.open);
        assert_eq!(c.prompts["a"], "x");
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
    fn efforts_history_and_plugins() {
        let cfg = |v| from_value(v, "test");
        let c = cfg(json!({
            "models": {"p/a": {}, "p/b": {"efforts": []}, "p/c": {"efforts": ["low", "high"]}}
        }))
        .unwrap();
        assert_eq!(c.model_efforts("p/a"), crate::DEFAULT_EFFORTS);
        assert!(c.model_efforts("p/b").is_empty());
        assert_eq!(c.model_efforts("p/c"), ["low", "high"]);
        assert_eq!(c.fallback_effort("p/a"), "medium");
        assert_eq!(c.fallback_effort("p/b"), "");
        assert_eq!(c.fallback_effort("p/c"), "low");
        for bad in [
            json!({"models": {"p/a": {"efforts": ["default"]}}}),
            json!({"models": {"p/a": {"efforts": ["bogus"]}}}),
            json!({"models": {"p/a": {"efforts": ["low", "low"]}}}),
        ] {
            let err = cfg(bad).unwrap_err();
            assert!(err.contains("p/a: efforts["), "{err}");
        }
        let err = cfg(json!({"history": {"hide": ["user", "bogus"]}})).unwrap_err();
        assert!(
            err.contains("history") || err.contains("unknown key"),
            "{err}"
        );

        let root = std::env::temp_dir().join(format!("tuse-plug-{}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        let d = root.join("plugins");
        std::fs::create_dir_all(d.join("alpha")).unwrap();
        std::fs::write(
            d.join("alpha/tuse-plugin.json"),
            r#"{"name":"alpha","version":"1.0.0"}"#,
        )
        .unwrap();
        std::fs::write(d.join("alpha/init.lua"), "return {}").unwrap();
        std::fs::create_dir_all(d.join("beta")).unwrap();
        std::fs::write(
            d.join("beta/tuse-plugin.json"),
            r#"{"name":"beta","version":"0.1.0","entrypoint":"sub/main.lua"}"#,
        )
        .unwrap();
        std::fs::create_dir_all(d.join("beta/sub")).unwrap();
        std::fs::write(d.join("beta/sub/main.lua"), "return {}").unwrap();
        let found = crate::discover_plugins_in(&d).unwrap();
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].name, "alpha");
        assert_eq!(found[1].name, "beta");
        std::fs::create_dir_all(d.join("bad")).unwrap();
        std::fs::write(
            d.join("bad/tuse-plugin.json"),
            r#"{"name":"bad","entrypoint":"../x.lua"}"#,
        )
        .unwrap();
        assert!(crate::discover_plugins_in(&d).is_err());
        std::fs::remove_dir_all(root).ok();
    }
}
