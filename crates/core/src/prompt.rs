use serde_json::{Value, json};
use std::path::Path;
use tuse_config::Config;

const SYSTEM: &str = include_str!("../prompts/system.md");

/// The built-in prompt, then the environment, the project rules and the `systems` of the config.
pub fn system_prompt(config: &Config, cwd: &Path, compacted: bool) -> String {
    let mut parts = vec![
        fill(SYSTEM, config).trim_end().to_string(),
        format!(
            "# Environment\nWorking directory: {}\nPlatform: {}",
            cwd.display(),
            platform()
        ),
    ];
    if let Ok(rules) = std::fs::read_to_string(cwd.join("AGENTS.md")) {
        parts.push(format!("# Project rules (AGENTS.md)\n{}", rules.trim_end()));
    }
    parts.extend(config.systems.iter().map(|s| s.trim_end().to_string()));
    let mut out = parts.join("\n\n");
    if compacted {
        out.push_str("\n\nEarlier parts of this conversation were compacted to save context: old tool calls and their outputs were removed and some later outputs were cut to their first line.");
    }
    out
}

fn platform() -> &'static str {
    static P: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    P.get_or_init(|| {
        std::process::Command::new("uname")
            .arg("-sm")
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_default()
    })
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
