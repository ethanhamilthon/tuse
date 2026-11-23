use serde_json::{Value, json};
use tuse_config::Config;

const SYSTEM: &str = include_str!("../prompts/system.md");

pub fn system_prompt(config: &Config, compacted: bool) -> String {
    let mut parts = vec![SYSTEM.trim_end().to_string()];
    parts[0].push_str(&crate::helpers::listing(config, false));
    parts.extend(config.systems.iter().map(|s| s.trim_end().to_string()));
    let mut out = parts.join("\n\n");
    if compacted {
        out.push_str("\n\nEarlier parts of this conversation were compacted to save context: old tool calls and their outputs were removed and some later outputs were cut to their first line.");
    }
    out
}

fn tool(name: &str, description: &str, properties: Value, required: &[&str]) -> Value {
    json!({
        "type": "function",
        "function": {
            "name": name,
            "description": description,
            "parameters": {"type": "object", "properties": properties, "required": required}
        }
    })
}

pub fn tools() -> Vec<Value> {
    vec![tool(
        "lua",
        "Run Lua 5.4 code in the session's persistent VM (globals stay between calls). Returns what the code prints plus the values it returns (a lone expression returns itself), or an error with a traceback. Prefer read() for file contents, grep() for search, and write()/edit()/append()/delete() for file changes so the user sees diffs. Use sh(cmd) for running programs and processing their output, not as the default for file operations; use sh([=[…]=]) when a shell command has quotes, backslashes, or regexes.",
        json!({
            "code": {"type": "string", "description": "The Lua code to run."}
        }),
        &["code"],
    )]
}
