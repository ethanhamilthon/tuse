use serde_json::{Value, json};
use tuse_config::Config;

const SYSTEM: &str = include_str!("../prompts/system.md");

pub fn system_prompt(config: &Config, compacted: bool, mode: crate::Mode) -> String {
    let mut parts = vec![fill(SYSTEM, config).trim_end().to_string()];
    parts[0].push_str(&crate::helpers::listing(config, mode, false));
    let cfg_mode = mode.into();
    parts.extend(
        config
            .systems
            .iter()
            .filter(|s| s.matches(cfg_mode))
            .map(|s| s.text().trim_end().to_string()),
    );
    let mut out = parts.join("\n\n");
    if compacted {
        out.push_str("\n\nEarlier parts of this conversation were compacted to save context: old tool calls and their outputs were removed and some later outputs were cut to their first line.");
    }
    out
}

/// Fills `{tool_timeout}`.
fn fill(text: &str, config: &Config) -> String {
    text.replace("{tool_timeout}", &config.settings.tool_timeout.to_string())
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

pub fn tools(config: &Config) -> Vec<Value> {
    vec![tool(
        "lua",
        "Run Lua 5.4 code in the session's persistent VM (globals stay between calls). Returns what the code prints plus the values it returns (a lone expression returns itself), or an error with a traceback. Files and search: read(), write(), edit(), grep(); programs: sh(cmd).",
        json!({
            "code": {"type": "string", "description": "The Lua code to run."},
            "title": {"type": "string", "description": "What this call does, in a few words and the user's language, e.g. \"Running the tests\". Shown to the user while it runs."},
            "timeout": {"type": "integer", "description": format!("Seconds before the call is stopped; default {}, no upper limit.", config.settings.tool_timeout)}
        }),
        &["code", "title"],
    )]
}
