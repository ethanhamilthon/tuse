use crate::{Event, Session, host::last_line};
use std::time::{Duration, Instant};
use tuse_config::Config;
use tuse_py::{Ctx, NEVER, Python};

pub fn install(s: &Session, py: &mut Python, config: &Config) {
    let ctx = Ctx::new(
        &NEVER,
        Instant::now() + Duration::from_secs(config.settings.hook_timeout.max(1)),
    );
    for (name, f) in &config.functions {
        let mut src = format!("def {name}({}):\n", f.args);
        for line in f.run.lines() {
            src.push_str("    ");
            src.push_str(line);
            src.push('\n');
        }
        let out = py.run(&src, &ctx, vec![]);
        if !out.ok {
            s.emit(Event::Notice(format!(
                "function {name}: {}",
                last_line(&out.text)
            )));
        }
    }
}

/// The functions the agent is told about; `show_agent: false` ones stay callable but unlisted.
pub fn listing(config: &Config, full: bool) -> String {
    let shown: Vec<_> = config
        .functions
        .iter()
        .filter(|(_, f)| f.show_agent)
        .collect();
    if shown.is_empty() {
        return String::new();
    }
    let mut out = String::from("\n\n# Your functions in `python`");
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
                    run: "return 1".into(),
                    show_agent,
                    ..Default::default()
                },
            );
        }
        for full in [false, true] {
            let text = listing(&config, full);
            assert!(text.contains("`seen("), "{text}");
            assert!(!text.contains("secret"), "{text}");
        }
        config.functions.get_mut("seen").unwrap().show_agent = false;
        assert!(listing(&config, false).is_empty());
    }
}
