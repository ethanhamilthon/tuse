const TOPICS: &[(&str, &str, &str)] = &[
    (
        "README.md",
        "Overview, install, usage, hotkeys, config, benchmark (long; read in slices)",
        include_str!("../../../README.md"),
    ),
    (
        "tool",
        "Lua tool, limits, timeouts and modules",
        include_str!("../../../docs/tool.md"),
    ),
    (
        "functions",
        "Built-in Lua functions (session, read, write, append, delete, edit, grep, sh, show, pick, ask, llm, reload, task_*, docs)",
        include_str!("../../../docs/functions.md"),
    ),
    (
        "json",
        "tuse json: JSON-RPC 2.0 over stdio for editors and other clients",
        include_str!("../../../docs/json.md"),
    ),
    (
        "commands",
        "CLI flags, TUI slash commands, steering and /trace",
        include_str!("../../../docs/commands.md"),
    ),
    (
        "env",
        "Environment variables and config file lookup",
        include_str!("../../../docs/env.md"),
    ),
    (
        "config",
        "Config modules, keys, compaction and files",
        include_str!("../../../docs/config.md"),
    ),
    (
        "prompts",
        "#prompts, @files and [Image #N] in a message: the hidden context blocks",
        include_str!("../../../docs/prompts.md"),
    ),
    (
        "plugins",
        "Plugins from GitHub: plugins = {...}, opts, merging, Git checkouts",
        include_str!("../../../docs/plugins.md"),
    ),
    (
        "hooks",
        "Event hooks",
        include_str!("../../../docs/hooks.md"),
    ),
    (
        "theme",
        "The 10-color theme and settings.ui",
        include_str!("../../../docs/theme.md"),
    ),
];

fn disabled_notice(disabled: &[&str]) -> String {
    let mut names = disabled.to_vec();
    names.sort_unstable();
    format!("Disabled in this session: {}.\n\n", names.join(", "))
}

fn topic_list(disabled: &[&str]) -> String {
    let mut out = String::new();
    if !disabled.is_empty() {
        out.push_str(&disabled_notice(disabled));
    }
    out.push_str("Available docs topics:\n");
    for (name, summary, _) in TOPICS {
        if *name == "functions" && !disabled.is_empty() {
            let mut items: Vec<&str> = [
                "session", "read", "write", "append", "delete", "edit", "grep", "sh", "show",
                "pick", "ask", "llm", "reload", "docs",
            ]
            .into_iter()
            .filter(|n| !disabled.contains(n))
            .collect();
            if ["task_get", "task_stop", "task_list"]
                .iter()
                .any(|n| !disabled.contains(n))
            {
                items.push("task_*");
            }
            if items.is_empty() {
                out.push_str("- functions — Built-in Lua functions\n");
            } else {
                out.push_str(&format!(
                    "- functions — Built-in Lua functions ({})\n",
                    items.join(", ")
                ));
            }
        } else {
            out.push_str(&format!("- {name} — {summary}\n"));
        }
    }
    if !disabled.contains(&"docs") {
        out.push_str("\nCall docs(\"<topic>\") to read a section.");
    }
    out
}

pub fn docs(topic: Option<&str>) -> String {
    docs_for(topic, &[])
}

pub fn docs_for(topic: Option<&str>, disabled: &[&str]) -> String {
    let Some(raw) = topic.map(str::trim).filter(|s| !s.is_empty()) else {
        return topic_list(disabled);
    };
    let key = raw.to_ascii_lowercase();
    let key = key.strip_suffix(".md").unwrap_or(&key);
    if let Some((name, _, body)) = TOPICS
        .iter()
        .find(|(name, _, _)| name.trim_end_matches(".md").eq_ignore_ascii_case(key))
    {
        let text = body.trim_end();
        if disabled.is_empty() {
            return text.to_string();
        }
        return match *name {
            "functions" => render_functions(disabled),
            "tool" => render_tool(disabled),
            _ => format!("{}{text}", disabled_notice(disabled)),
        };
    }
    format!("Unknown docs topic \"{raw}\".\n\n{}", topic_list(disabled))
}

fn render_functions(disabled: &[&str]) -> String {
    let has = |n: &str| !disabled.contains(&n);
    let mut out = String::from(
        "# Functions\n\nBuilt-in host functions are globals in the agent's `lua` tool and are also exposed on `require(\"tuse\")`. Errors are plain Lua errors (catchable with `pcall`). File paths accept absolute paths, `~/...`, or paths relative to the session working directory.",
    );
    let mut prefs = crate::prompt::prefer_sentence(disabled, false);
    if has("sh") {
        prefs.push_str(" Use `sh` for running programs and processing their output.");
    }
    if !prefs.is_empty() {
        out.push_str("\n\n");
        out.push_str(prefs.trim_start());
    }
    let mut current = "";
    for (sec, name, line) in crate::prompt::FUNC_SPECS {
        if !has(name) {
            continue;
        }
        if *sec != current {
            out.push_str(&format!("\n\n## {sec}\n"));
            current = sec;
        }
        out.push('\n');
        out.push_str(line);
    }
    out
}

fn render_tool(disabled: &[&str]) -> String {
    let has = |n: &str| !disabled.contains(&n);
    let mut lines: Vec<String> = vec![
        "# The `lua` tool".into(),
        String::new(),
        "The agent has a single tool, `lua`, taking one required string argument `code` (Lua 5.4 source).".into(),
        String::new(),
        "## Tool call contract".into(),
        "- **Arguments:** JSON object `{\"code\": \"...\"}`. Unknown tools, invalid JSON, or missing/non-string `code` fail with `code = 1` without starting a Lua job.".into(),
        "- **Compilation and output:** Tuse first tries `return <code>` so a lone expression returns its value; otherwise compiles `<code>` as statements. Captured `print(...)` and `io.write(...)` come first, followed by formatted non-nil return values. Output is uncapped.".into(),
        "- **Exit status:** `0` on success, `1` on error or timeout, `nil` on cancel.".into(),
        String::new(),
        "## VM lifetime, timeouts, and scheduling".into(),
        "- **Persistent VM:** One Lua 5.4 VM runs per session. Globals persist between `lua` calls (use `local` for scratch variables).".into(),
        if has("spawn") {
            "- **120-second deadline:** Each `lua` call has a 120-second deadline. Start long jobs with `spawn(fn, name)` (no deadline).".into()
        } else {
            "- **120-second deadline:** Each `lua` call has a 120-second deadline.".into()
        },
        "- **Standard library:** `string`, `table`, `math`, `utf8`, `io`, `os` (without `os.exit`), `coroutine`, `package`, `load`, `loadfile`, `dofile`, and `require`. The `debug` library is unavailable.".into(),
    ];
    let extras: Vec<&str> = [
        has("json").then_some("`json.encode`/`json.decode`"),
        has("inspect").then_some("`inspect(value)`"),
    ]
    .into_iter()
    .flatten()
    .collect();
    if !extras.is_empty() {
        lines.push(format!("- **Added globals:** {}.", extras.join(", ")));
    }
    let waits: Vec<String> = ["sh", "sleep", "pick", "ask", "reload", "llm"]
        .into_iter()
        .filter(|n| has(n))
        .map(|n| format!("`{n}`"))
        .collect();
    if !waits.is_empty() {
        lines.push(format!(
            "- **Cooperative yielding:** {} yield cooperatively so other coroutines keep running.",
            waits.join(", ")
        ));
    }
    let paused: Vec<String> = ["pick", "ask", "reload"]
        .into_iter()
        .filter(|n| has(n))
        .map(|n| format!("`{n}`"))
        .collect();
    if !paused.is_empty() {
        lines.push(format!(
            "- **Paused deadlines:** {} pause the calling coroutine's remaining deadline while waiting.",
            paused.join(", ")
        ));
    }
    let active_tools: Vec<&str> = crate::prompt::FUNC_SPECS
        .iter()
        .filter(|(_, name, _)| {
            matches!(
                *name,
                "read" | "write" | "append" | "delete" | "edit" | "grep" | "sh"
            ) && has(name)
        })
        .map(|(_, _, line)| *line)
        .collect();
    if !active_tools.is_empty() {
        lines.push(String::new());
        lines.push("## Available file, search, and shell functions".into());
        lines.extend(active_tools.into_iter().map(String::from));
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn docs_lists_finds_and_filters_disabled_builtins() {
        let index = docs(None);
        for (name, _, _) in TOPICS {
            assert!(index.contains(&format!("- {name} —")), "missing {name}");
        }
        assert_eq!(index, docs(Some("  ")));
        let hooks = docs(Some("hooks"));
        assert!(hooks.starts_with("# Hooks"));
        assert_eq!(hooks, docs(Some(" HOOKS ")));
        assert_eq!(hooks, docs(Some("Hooks.MD")));
        let readme = docs(Some(" Readme.md "));
        assert!(readme.starts_with("# Tuse"));
        assert_eq!(readme, docs(Some("readme")));
        let err = docs(Some("nope"));
        assert!(err.starts_with("Unknown docs topic \"nope\"."));
        assert!(err.contains("- config —"));

        let f = docs_for(Some("functions"), &["sh", "delete", "docs"]);
        assert!(!f.contains("`sh(") && !f.contains("`sh`"), "{f}");
        assert!(!f.contains("`delete(") && !f.contains("`delete`"), "{f}");
        assert!(
            !f.contains("docs(\"config\")") && !f.contains("`docs("),
            "{f}"
        );
        assert!(f.contains("`read(") && f.contains("`write("));

        let idx_no_docs = docs_for(None, &["docs"]);
        assert!(!idx_no_docs.contains("Call docs("), "{idx_no_docs}");

        let t = docs_for(Some("tool"), &["sh", "delete"]);
        assert!(!t.contains("`sh(") && !t.contains("`sh`"), "{t}");
        assert!(!t.contains("`delete(") && !t.contains("`delete`"), "{t}");
        assert!(t.contains("`read("));

        let c = docs_for(Some("config"), &["sh", "delete"]);
        assert!(
            c.starts_with("Disabled in this session: delete, sh."),
            "{c}"
        );
    }
}
