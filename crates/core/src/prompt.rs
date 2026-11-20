use serde_json::{Value, json};
use tuse_config::Config;

const SYSTEM: &str = include_str!("../prompts/system.md");

pub fn system_prompt(config: &Config, compacted: bool) -> String {
    let mut parts = vec![fill(SYSTEM, config).trim_end().to_string()];
    parts.extend(config.systems.iter().map(|s| s.trim_end().to_string()));
    let mut out = parts.join("\n\n");
    if compacted {
        out.push_str("\n\nEarlier parts of this conversation were compacted to save context: old tool calls and their outputs were removed and some later outputs were cut to their first line.");
    }
    out
}

/// `4096` bytes as `4 KB`, so the number in the prompt reads like the setting.
fn size(n: usize) -> String {
    if n >= 1024 && n.is_multiple_of(1024) {
        format!("{} KB", n / 1024)
    } else {
        format!("{n} bytes")
    }
}

/// Fills `{max_output}` and `{tool_timeout}`.
fn fill(text: &str, config: &Config) -> String {
    text.replace("{max_output}", &size(config.settings.max_output))
        .replace("{tool_timeout}", &config.settings.tool_timeout.to_string())
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
        "python",
        &format!(
            "Run Python code in a persistent REPL (variables and imports stay between calls). Returns what the code prints plus the value of the last expression, or a traceback; output over {} is cut in the middle. Files and search: read(), write(), edit(), grep(); programs: sh(cmd).",
            size(config.settings.max_output)
        ),
        json!({
            "code": {"type": "string", "description": "The Python code to run."},
            "title": {"type": "string", "description": "What this call does, in a few words and the user's language, e.g. \"Running the tests\". Shown to the user while it runs."},
            "timeout": {"type": "integer", "description": format!("Seconds before the call is stopped. Default {}.", config.settings.tool_timeout)}
        }),
        &["code", "title"],
    )]
}
