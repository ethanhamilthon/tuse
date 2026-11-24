use serde_json::{Value, json};
use tuse_config::Config;

fn join_clauses(items: &[String]) -> String {
    match items {
        [] => String::new(),
        [a] => a.clone(),
        [a, b] => format!("{a} and {b}"),
        [rest @ .., last] => format!("{}, and {last}", rest.join(", ")),
    }
}

pub(crate) fn prefer_sentence(disabled: &[&str], with_parens: bool) -> String {
    let has = |n: &str| !disabled.contains(&n);
    let fmt = |n: &str| {
        if with_parens {
            format!("{n}()")
        } else {
            format!("`{n}`")
        }
    };
    let mut parts = vec![];
    if has("read") {
        parts.push(format!("{} for file contents", fmt("read")));
    }
    if has("grep") {
        parts.push(format!("{} for search", fmt("grep")));
    }
    let writes: Vec<String> = ["write", "edit", "append", "delete"]
        .into_iter()
        .filter(|n| has(n))
        .map(fmt)
        .collect();
    if !writes.is_empty() {
        parts.push(format!(
            "{} for file changes so the user sees diffs",
            writes.join("/")
        ));
    }
    if parts.is_empty() {
        String::new()
    } else {
        format!(" Prefer {}.", join_clauses(&parts))
    }
}

pub(crate) const FUNC_SPECS: &[(&str, &str, &str)] = &[
    (
        "File Functions",
        "read",
        "- `read(path)` / `read(path, {lines=true, offset=1, limit=2000})`: returns raw text by default, or an image (png, jpg, gif, webp) that you then see. Pass `{lines=true}` for numbered, paginated lines (ending with the next `offset`).",
    ),
    (
        "File Functions",
        "write",
        "- `write(path, content)`: create a file or replace it completely; missing folders are created.",
    ),
    (
        "File Functions",
        "append",
        "- `append(path, text)`: append text to a file (creating it if missing) and show the change as a diff.",
    ),
    (
        "File Functions",
        "delete",
        "- `delete(path)`: delete a single file and show its removal as a diff (does not delete directories).",
    ),
    (
        "File Functions",
        "edit",
        "- `edit(path, old, new, {all=false})`: replace exact text in a file. `old` must be unique unless `all` is set; read the file first. A wrong `old` raises an error.",
    ),
    (
        "File Functions",
        "grep",
        "- `grep(pattern, {path=\".\", glob=nil, ignore_case=false, literal=false, context=0, limit=100})`: search file contents with a regex (or plain text with `literal=true`); `path` can be a string or list of paths. Returns `path:line: text` per match and respects `.gitignore`.",
    ),
    (
        "Shell, Timing, and Background Tasks",
        "sh",
        "- `sh(cmd, {timeout=nil, cwd=nil, env={}})`: runs `bash -c cmd` in the working directory; returns a table with `.out` (stdout and stderr merged), `.code` (`nil` when killed), `.ok`, `.timed_out` and `:lines()`. Use Lua long strings like `sh([=[grep -E \"foo\\\\bar\" f]=])` for commands with quotes, backslashes, or regexes so Lua does not interpret shell escapes. Returned as a value it shows its output and exit code. Shell state does not carry over between calls. At its timeout or the call's the command is killed.",
    ),
    (
        "Shell, Timing, and Background Tasks",
        "sleep",
        "- `sleep(seconds)`: waits cooperatively while other tasks and hooks keep running.",
    ),
    (
        "Shell, Timing, and Background Tasks",
        "spawn",
        "- `spawn(fn, name)`: runs `fn` in the background as a task in this same VM (it sees your globals); returns its ID. A task has no deadline; its `print` goes to its log, and you get a message when it ends.",
    ),
    (
        "Shell, Timing, and Background Tasks",
        "task_get",
        "- `task_get(id, lines=50)`: return `{ id, name, status, started, ended, output }` with the log tail in `.output`.",
    ),
    (
        "Shell, Timing, and Background Tasks",
        "task_stop",
        "- `task_stop(id)`: stop a running background task and return its task info.",
    ),
    (
        "Shell, Timing, and Background Tasks",
        "task_list",
        "- `task_list()`: list all tasks in the session with `.id`, `.name`, and `.status` (`running`, `done`, `failed`, `stopped`).",
    ),
    (
        "Interactive UI",
        "show",
        "- `show(name, text, pin)`: render or update a UI-only Markdown block by name (`pin=true` keeps it at the end of the chat after new messages; `show(name, \"\", false)` removes it).",
    ),
    (
        "Interactive UI",
        "pick",
        "- `pick(options)`: show a list of strings to the user and return the selected string, or `nil` on Esc (pauses your timeout while waiting).",
    ),
    (
        "Interactive UI",
        "ask",
        "- `ask(text)`: show a Markdown question to the user and return their typed answer, or `nil` on Esc (pauses your timeout while waiting).",
    ),
    (
        "Control, Model, Docs, and Utilities",
        "llm",
        "- `llm(prompt, model, effort)`: send one prompt (no tools, no history) to a model and return its text; `model` and `effort` default to the session's.",
    ),
    (
        "Control, Model, Docs, and Utilities",
        "reload",
        "- `reload()`: reload the config like `/reload`; raises if it is invalid.",
    ),
    (
        "Control, Model, Docs, and Utilities",
        "set_title",
        "- `set_title(text)`: update the session title (up to 80 characters).",
    ),
    (
        "Control, Model, Docs, and Utilities",
        "session",
        "- `session()`: return a fresh snapshot of runtime session data (`mode`, `configpath`, `cwd`, `home`, `id`, `name`, `model`, `provider`, `effort`, `ctx`, `ctx_size`, `usage`, `model_info`, `steps`, `turns`, `trace`).",
    ),
    (
        "Control, Model, Docs, and Utilities",
        "docs",
        "- `docs(topic)`: call `docs()` to list documentation topics, or `docs(\"<topic>\")` to read one.",
    ),
    (
        "Control, Model, Docs, and Utilities",
        "json",
        "- `json.encode(value)` / `json.decode(text)`: encode a Lua value to compact JSON or decode JSON to Lua.",
    ),
    (
        "Control, Model, Docs, and Utilities",
        "inspect",
        "- `inspect(value)`: format any Lua value into a readable string.",
    ),
];

pub fn system_prompt(config: &Config, compacted: bool) -> String {
    let disabled = config.disabled_builtins();
    let has = |n: &str| !disabled.contains(&n);
    let mut base = String::from(
        "You are running inside Tuse, a coding agent.\n\n# Tool\nYou have one tool, `lua`: it runs Lua 5.4 code in the session's persistent VM, so globals stay between calls (use `local` for scratch values). You get what the code prints plus the values it returns (a lone expression returns itself), or an error with a traceback. Nothing is cut, so keep big outputs short yourself. A call is stopped after 120 seconds",
    );
    if has("spawn") {
        base.push_str("; start a long job in the background with `spawn`");
    }
    base.push_str(".\n\nThe full Lua standard library is there (`string`, `table`, `math`, `io`, `os`, `utf8`, `coroutine`)");
    match (has("json"), has("inspect")) {
        (true, true) => base.push_str(", plus `json.encode`/`json.decode` and `inspect(value)`."),
        (true, false) => base.push_str(", plus `json.encode`/`json.decode`."),
        (false, true) => base.push_str(", plus `inspect(value)`."),
        (false, false) => base.push('.'),
    }
    match (has("grep"), has("sh")) {
        (true, true) => {
            base.push_str(" Lua patterns are not regexes: use `grep` or `sh` for those.")
        }
        (true, false) => base.push_str(" Lua patterns are not regexes: use `grep` for those."),
        (false, true) => base.push_str(" Lua patterns are not regexes: use `sh` for those."),
        (false, false) => {}
    }
    base.push_str(
        "\n\n# Rules\n- Prefer short, multi-step scripts over one big call. Create a file and extend it in small chunks instead of writing a 1000-line file in a single call.",
    );

    let bullets: Vec<&str> = FUNC_SPECS
        .iter()
        .filter(|(_, name, _)| !matches!(*name, "json" | "inspect" | "set_title") && has(name))
        .map(|(_, _, line)| *line)
        .collect();
    if !bullets.is_empty() {
        base.push_str("\n\n# Functions in `lua`\nThey are Lua globals, not tools: call them inside `lua` code. Errors are Lua errors (catch them with `pcall`).");
        base.push_str(&prefer_sentence(&disabled, false));
        if has("sh") {
            base.push_str(" Use `sh` for running programs and processing their output, not as the default for file operations.");
        }
        for b in bullets {
            base.push('\n');
            base.push_str(b);
        }
    }

    let mut parts = vec![base];
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

pub fn tools(config: &Config) -> Vec<Value> {
    let disabled = config.disabled_builtins();
    let mut desc = String::from(
        "Run Lua 5.4 code in the session's persistent VM (globals stay between calls). Returns what the code prints plus the values it returns (a lone expression returns itself), or an error with a traceback.",
    );
    desc.push_str(&prefer_sentence(&disabled, true));
    if !disabled.contains(&"sh") {
        desc.push_str(" Use sh(cmd) for running programs and processing their output, not as the default for file operations; use sh([=[…]=]) when a shell command has quotes, backslashes, or regexes.");
    }
    vec![tool(
        "lua",
        &desc,
        json!({
            "code": {"type": "string", "description": "The Lua code to run."}
        }),
        &["code"],
    )]
}

#[cfg(test)]
mod tests {
    use super::*;
    use tuse_config::FunctionKind;

    #[test]
    fn custom_functions_are_undiscoverable_and_disabled_builtins_are_omitted() {
        let mut config = Config::default();
        config
            .functions
            .insert("secret".into(), FunctionKind::Custom);
        config.functions.insert("sh".into(), FunctionKind::Disabled);
        config
            .functions
            .insert("delete".into(), FunctionKind::Disabled);
        config
            .functions
            .insert("docs".into(), FunctionKind::Disabled);
        config
            .functions
            .insert("task_get".into(), FunctionKind::Disabled);
        let sys = system_prompt(&config, false);
        assert!(!sys.contains("secret"), "{sys}");
        assert!(!sys.contains("`sh`") && !sys.contains("`sh("), "{sys}");
        assert!(
            !sys.contains("`delete`") && !sys.contains("`delete("),
            "{sys}"
        );
        assert!(!sys.contains("`docs("), "{sys}");
        assert!(!sys.contains("`task_get("), "{sys}");
        assert!(sys.contains("`read(") && sys.contains("`write("));

        let desc = tools(&config)[0]["function"]["description"]
            .as_str()
            .unwrap()
            .to_string();
        assert!(
            !desc.contains("sh(") && !desc.contains("delete()"),
            "{desc}"
        );
        assert!(
            desc.contains("read()") && desc.contains("write()"),
            "{desc}"
        );
    }
}
