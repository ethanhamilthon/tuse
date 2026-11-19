use serde_json::{Value, json};
use tuse_config::Config;

pub fn system_prompt(config: &Config, compacted: bool) -> String {
    let mut out = String::new();
    for sys in &config.systems {
        if !out.is_empty() {
            out.push_str("\n\n");
        }
        out.push_str(fill(sys, config).trim_end());
    }
    if compacted {
        out.push_str("\n\nEarlier parts of this conversation were compacted to save context: old tool calls and their outputs were removed and some later outputs were cut to their first line.");
    }
    out
}

/// Fills `{max_output}` and `{tool_timeout}` (and their pre-0.4 names) in a `systems` entry.
fn fill(sys: &str, config: &Config) -> String {
    let (max, timeout) = (
        config.settings.max_output.to_string(),
        config.settings.tool_timeout.to_string(),
    );
    sys.replace("{max_output}", &max)
        .replace("{max_bash_output}", &max)
        .replace("{tool_timeout}", &timeout)
        .replace("{bash_timeout}", &timeout)
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
    vec![
        tool(
            "python",
            "Run Python code in a persistent REPL (variables and imports stay between calls). Returns what the code prints plus the value of the last expression, or a traceback. Run programs with sh(cmd).",
            json!({
                "code": {"type": "string", "description": "The Python code to run."},
                "timeout": {"type": "integer", "description": format!("Seconds before the call is stopped. Default {}.", config.settings.tool_timeout)}
            }),
            &["code"],
        ),
        tool(
            "read",
            "Read a text file with line numbers, or an image (png, jpg, gif, webp) so you can see it. Long files come in pages: use offset and limit.",
            json!({
                "path": {"type": "string", "description": "File path: absolute, ~/ or relative to the working directory."},
                "offset": {"type": "integer", "description": "First line to read, 1-based. Default 1."},
                "limit": {"type": "integer", "description": "Number of lines. Default 2000."}
            }),
            &["path"],
        ),
        tool(
            "write",
            "Create a file or replace it completely. Creates missing folders. For a change inside an existing file use edit.",
            json!({
                "path": {"type": "string", "description": "File path."},
                "content": {"type": "string", "description": "The whole new content."}
            }),
            &["path", "content"],
        ),
        tool(
            "edit",
            "Replace text in a file. `old` must match exactly (including whitespace) and be unique unless replace_all is set. Read the file first.",
            json!({
                "path": {"type": "string", "description": "File path."},
                "old": {"type": "string", "description": "The exact text to replace."},
                "new": {"type": "string", "description": "The replacement."},
                "replace_all": {"type": "boolean", "description": "Replace every occurrence. Default false."}
            }),
            &["path", "old", "new"],
        ),
    ]
}
