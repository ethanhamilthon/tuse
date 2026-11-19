use tuse_config::Config;

pub fn system_prompt(config: &Config, compacted: bool) -> String {
    let mut out = String::new();
    for sys in &config.systems {
        if !out.is_empty() {
            out.push_str("\n\n");
        }
        out.push_str(fill(sys, config).trim_end());
    }
    if !config.bins.is_empty() {
        out.push_str("\n\n# Commands\nShort commands available in bash:\n");
        for (name, cmd) in &config.bins {
            out.push_str(&format!("- `{name}`: runs `{cmd}`\n"));
        }
    }
    if compacted {
        out.push_str("\n\nEarlier parts of this conversation were compacted to save context: old tool calls and their outputs were removed and some later outputs were cut to their first line.");
    }
    out
}

/// Fills `{max_bash_output}` and `{bash_timeout}` in a `systems` entry.
fn fill(sys: &str, config: &Config) -> String {
    sys.replace(
        "{max_bash_output}",
        &config.settings.max_bash_output.to_string(),
    )
    .replace("{bash_timeout}", &config.settings.bash_timeout.to_string())
}

pub fn bash_tool(config: &Config) -> serde_json::Value {
    serde_json::json!({
        "type": "function",
        "function": {
            "name": "bash",
            "description": "Run a command with `bash -c` in the working directory. Returns merged stdout and stderr.",
            "parameters": {
                "type": "object",
                "properties": {
                    "command": {"type": "string", "description": "The bash command to run."},
                    "timeout": {"type": "integer", "description": format!("Seconds before the command is killed. Default {}.", config.settings.bash_timeout)}
                },
                "required": ["command"]
            }
        }
    })
}
