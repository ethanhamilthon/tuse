use serde_json::{Value, json};
use tuse_api::ToolDef;
use tuse_config::Config;

const SYSTEM: &str = include_str!("../prompts/system.md");

pub fn system_prompt(config: &Config, compacted: bool) -> String {
    let mut parts = vec![SYSTEM.trim_end().to_string()];
    parts[0].push_str(&listing(config, false));
    parts.extend(config.systems.iter().map(|s| s.trim_end().to_string()));
    let mut out = parts.join("\n\n");
    if compacted {
        out.push_str("\n\nEarlier parts of this conversation were compacted to save context: old tool calls and their outputs were removed and some later outputs were cut to their first line.");
    }
    out
}

fn tool(name: &str, description: &str, properties: Value, required: &[&str]) -> ToolDef {
    ToolDef {
        name: name.to_string(),
        description: description.to_string(),
        parameters: json!({"type": "object", "properties": properties, "required": required}),
    }
}

pub fn tools() -> Vec<ToolDef> {
    vec![tool(
        "lua",
        "Run Lua 5.4 code in the session's persistent VM (globals stay between calls). Returns what the code prints plus the values it returns (a lone expression returns itself), or an error with a traceback. Prefer read() for file contents, grep() for search, and write()/edit()/append()/delete() for file changes so the user sees diffs. Use sh(cmd) for running programs and processing their output, not as the default for file operations; use sh([=[…]=]) when a shell command has quotes, backslashes, or regexes.",
        json!({
            "code": {"type": "string", "description": "The Lua code to run."}
        }),
        &["code"],
    )]
}

pub fn listing(config: &Config, full: bool) -> String {
    let shown: Vec<_> = config
        .functions
        .iter()
        .filter(|(_, f)| f.show_agent)
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
            let text = listing(&config, full);
            assert!(text.contains("`seen("), "{text}");
            assert!(!text.contains("secret"), "{text}");
        }
        config.functions.get_mut("seen").unwrap().show_agent = false;
        assert!(listing(&config, false).is_empty());
    }
}
