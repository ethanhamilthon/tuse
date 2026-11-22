use tuse_config::Config;

/// The functions the agent is told about; `show_agent: false` ones stay callable but unlisted.
pub fn listing(config: &Config, mode: crate::Mode, full: bool) -> String {
    let mode = mode.into();
    let shown: Vec<_> = config
        .functions
        .iter()
        .filter(|(_, f)| f.show_agent && f.matches(mode))
        .collect();
    if shown.is_empty() {
        return String::new();
    }
    let mut out = String::from("\n\n# Your functions in `lua`");
    for (name, f) in shown.iter().take(if full { usize::MAX } else { 30 }) {
        let desc: String = f
            .description
            .chars()
            .take(if full { usize::MAX } else { 200 })
            .collect();
        out.push_str(&format!("\n- `{name}({})`: {desc}", f.args));
    }
    if !full && shown.len() > 30 {
        out.push_str(&format!(
            "\nand {} more, see `docs(\"functions\")`",
            shown.len() - 30
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hidden_functions_are_not_listed_but_stay_defined() {
        let mut config = Config::default();
        for (name, show_agent) in [("seen", true), ("secret", false)] {
            config.functions.insert(
                name.into(),
                tuse_config::Function {
                    description: "d".into(),
                    show_agent,
                    ..Default::default()
                },
            );
        }
        for full in [false, true] {
            let text = listing(&config, crate::Mode::Tui, full);
            assert!(text.contains("`seen("), "{text}");
            assert!(!text.contains("secret"), "{text}");
        }
        config.functions.get_mut("seen").unwrap().show_agent = false;
        assert!(listing(&config, crate::Mode::Tui, false).is_empty());
    }

    #[test]
    fn functions_filter_by_mode() {
        let mut config = Config::default();
        for (name, modes) in [
            ("all", vec![]),
            ("only_tui", vec![tuse_config::Mode::Tui]),
            ("only_json", vec![tuse_config::Mode::Json]),
        ] {
            config.functions.insert(
                name.into(),
                tuse_config::Function {
                    description: "d".into(),
                    modes,
                    ..Default::default()
                },
            );
        }
        let tui = listing(&config, crate::Mode::Tui, false);
        assert!(tui.contains("`all(") && tui.contains("`only_tui(") && !tui.contains("only_json"));
        let json = listing(&config, crate::Mode::Json, true);
        assert!(
            json.contains("`all(") && json.contains("`only_json(") && !json.contains("only_tui")
        );
    }
}
