use std::path::Path;
use tuse_config::Config;

const INTRO: &str = "You are running inside Tuse, a minimal terminal coding agent.

# Tools
You have one tool, `bash`: it runs `bash -c COMMAND` as a fresh process in the working directory, so shell state (cd, variables) does not carry over between calls. stdout and stderr come back merged, cut to {max} bytes in the middle when longer; a call is killed after {timeout}s unless you pass a larger `timeout`. Run servers, watchers and other long-running processes with `tuse task run`, never with `&`.

# Tuse CLI
`TUSE_SESSION` is set to this session's ID, so these commands act on it:
- `tuse ask QUESTION OPTION...`: ask the user to pick an option (they can also type their own answer). Blocks and prints the answer. With `--async` it prints an ID right away and the answer arrives later as a message.
- `tuse task run [--name NAME] -- COMMAND...`: start a background task; prints its ID. You get a message when it exits.
- `tuse task get ID [--lines N]`: status and the last lines of output. `tuse task stop ID`, `tuse task list`.
- `tuse steer [-s ID] [--file PATH]... TEXT`: send a message to a session (as if the user typed it).";

pub fn system_prompt(config: &Config, cwd: &Path, compacted: bool) -> String {
    let s = &config.settings;
    let mut out = INTRO
        .replace("{max}", &s.max_bash_output.to_string())
        .replace("{timeout}", &s.bash_timeout.to_string());
    if !config.bins.is_empty() {
        out.push_str("\n\n# Commands\nShort commands available in bash:\n");
        for (name, cmd) in &config.bins {
            out.push_str(&format!("- `{name}`: runs `{cmd}`\n"));
        }
    }
    out.push_str(&format!(
        "\n\n# Environment\nWorking directory: {}\nPlatform: {} {}",
        cwd.display(),
        std::env::consts::OS,
        std::env::consts::ARCH
    ));
    if compacted {
        out.push_str("\n\nEarlier parts of this conversation were compacted to save context: old tool calls and their outputs were removed and some later outputs were cut to their first line.");
    }
    for sys in &config.systems {
        out.push_str("\n\n");
        out.push_str(sys.trim_end());
    }
    out
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
