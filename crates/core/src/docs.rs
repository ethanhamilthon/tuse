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
            let items: Vec<&str> = [
                "session", "read", "write", "append", "delete", "edit", "grep", "sh", "show",
                "pick", "ask", "llm", "reload", "docs",
            ]
            .into_iter()
            .filter(|n| !disabled.contains(n))
            .collect();
            let has_tasks = ["task_get", "task_stop", "task_list"]
                .iter()
                .any(|n| !disabled.contains(n));
            let mut list = items;
            if has_tasks {
                list.push("task_*");
            }
            if list.is_empty() {
                out.push_str("- functions — Built-in Lua functions\n");
            } else {
                out.push_str(&format!(
                    "- functions — Built-in Lua functions ({})\n",
                    list.join(", ")
                ));
            }
        } else {
            out.push_str(&format!("- {name} — {summary}\n"));
        }
    }
    out.push_str("\nCall docs(\"<topic>\") to read a section.");
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
            "functions" => filter_functions_doc(text, disabled),
            "tool" => filter_tool_doc(text, disabled),
            _ => format!("{}{text}", disabled_notice(disabled)),
        };
    }
    format!("Unknown docs topic \"{raw}\".\n\n{}", topic_list(disabled))
}

fn bullet_builtin(line: &str) -> Option<&'static str> {
    let rest = line.strip_prefix("- `")?;
    let name = rest.split(['(', '.', '`']).next()?;
    tuse_config::HOST_FUNCTIONS
        .iter()
        .copied()
        .find(|n| *n == name)
}

fn filter_functions_doc(body: &str, disabled: &[&str]) -> String {
    let has = |n: &str| !disabled.contains(&n);
    let mut sections: Vec<(Option<String>, Vec<String>)> = vec![];
    let mut cur_head: Option<String> = None;
    let mut cur_lines: Vec<String> = vec![];
    for line in body.lines() {
        if line.starts_with("## ") {
            sections.push((cur_head.take(), std::mem::take(&mut cur_lines)));
            cur_head = Some(line.to_string());
        } else {
            cur_lines.push(line.to_string());
        }
    }
    sections.push((cur_head, cur_lines));

    let mut out: Vec<String> = vec![];
    for (head, lines) in sections {
        match head.as_deref() {
            Some("## `session()`") if !has("session") => continue,
            Some("## Functions from Config") => {
                out.push(
                    "## Functions from Config\n\nSee `docs(\"config\")` for configuring `functions`."
                        .into(),
                );
                continue;
            }
            _ => {}
        }
        let mut kept: Vec<String> = vec![];
        let mut skipping_bullet = false;
        let mut kept_bullets = 0usize;
        let mut had_bullets = false;
        for line in lines {
            if line.starts_with("- `") {
                had_bullets = true;
                if let Some(name) = bullet_builtin(&line)
                    && !has(name)
                {
                    skipping_bullet = true;
                    continue;
                }
                skipping_bullet = false;
                kept_bullets += 1;
            } else if skipping_bullet && (line.starts_with("  ") || line.is_empty()) {
                continue;
            } else {
                skipping_bullet = false;
            }

            if line.starts_with("Prefer `read` for file contents") {
                let mut parts = vec![];
                if has("read") {
                    parts.push("`read` for file contents".to_string());
                }
                if has("grep") {
                    parts.push("`grep` for search".to_string());
                }
                let writes: Vec<String> = ["write", "edit", "append", "delete"]
                    .into_iter()
                    .filter(|n| has(n))
                    .map(|n| format!("`{n}`"))
                    .collect();
                if !writes.is_empty() {
                    parts.push(format!(
                        "{} for file changes so diffs are recorded and shown to the user",
                        writes.join("/")
                    ));
                }
                let mut rewritten = String::new();
                if !parts.is_empty() {
                    let joined = match parts.as_slice() {
                        [a] => a.clone(),
                        [a, b] => format!("{a} and {b}"),
                        [rest @ .., last] => format!("{}, and {last}", rest.join(", ")),
                        [] => String::new(),
                    };
                    rewritten.push_str(&format!("Prefer {joined}."));
                }
                if has("sh") {
                    if !rewritten.is_empty() {
                        rewritten.push(' ');
                    }
                    rewritten
                        .push_str("Use `sh` for running programs and processing their output.");
                }
                if !rewritten.is_empty() {
                    kept.push(rewritten);
                }
                continue;
            }
            if line.starts_with("`pick` and `ask` share a single FIFO queue") {
                match (has("pick"), has("ask")) {
                    (true, true) => kept.push(line),
                    (true, false) => {
                        kept.push(line.replace("`pick` and `ask` share", "`pick` uses"))
                    }
                    (false, true) => {
                        kept.push(line.replace("`pick` and `ask` share", "`ask` uses"))
                    }
                    (false, false) => {}
                }
                continue;
            }
            let mut line = line;
            if !has("sh") {
                line = line.replace("; list it with sh(\"ls\")", "");
            }
            if !has("read") {
                line = line.replace(
                    " (for example, `read(path, { lines = true, offset = 40 })`)",
                    "",
                );
            }
            kept.push(line);
        }
        if head.is_some() && had_bullets && kept_bullets == 0 {
            continue;
        }
        let body_part = kept.join("\n").trim().to_string();
        if let Some(h) = head {
            if !body_part.is_empty() {
                out.push(format!("{h}\n\n{body_part}"));
            }
        } else if !body_part.is_empty() {
            out.push(body_part);
        }
    }
    out.join("\n\n")
}

fn filter_tool_doc(body: &str, disabled: &[&str]) -> String {
    let has = |n: &str| !disabled.contains(&n);
    let mut out = Vec::new();
    let mut skip_indent = false;
    for line in body.lines() {
        if skip_indent {
            if line.starts_with("  ") {
                continue;
            }
            skip_indent = false;
        }
        if line.starts_with("- **`read(") && !has("read") {
            skip_indent = true;
            continue;
        }
        if line.starts_with("- **`grep(") && !has("grep") {
            skip_indent = true;
            continue;
        }
        if line.starts_with("- **`sh(") && !has("sh") {
            skip_indent = true;
            continue;
        }
        if line.starts_with("- **`write(") {
            let mut heads = vec![];
            if has("write") {
                heads.push("`write(path, content)`");
            }
            if has("append") {
                heads.push("`append(path, text)`");
            }
            if has("delete") {
                heads.push("`delete(path)`");
            }
            if has("edit") {
                heads.push("`edit(path, old, new, {all = false})`");
            }
            if heads.is_empty() {
                skip_indent = true;
                continue;
            }
            out.push(format!("- **{}:**", heads.join(" / ")));
            continue;
        }
        if line.starts_with("  - `write`, `append`, and `edit`") {
            let mut parts = vec![];
            let atomic: Vec<&str> = ["write", "append", "edit"]
                .into_iter()
                .filter(|n| has(n))
                .collect();
            if !atomic.is_empty() {
                let names: Vec<String> = atomic.iter().map(|n| format!("`{n}`")).collect();
                parts.push(format!(
                    "{} write atomically (temp file in target directory, preserving existing file permissions, flushing to disk, then renaming).",
                    names.join(", ")
                ));
            }
            if has("write") || has("append") {
                let wa: Vec<String> = ["write", "append"]
                    .into_iter()
                    .filter(|n| has(n))
                    .map(|n| format!("`{n}`"))
                    .collect();
                parts.push(format!(
                    "{} create missing parent folders and reject directories.",
                    wa.join(" and ")
                ));
            }
            if has("delete") {
                parts.push("`delete` removes a single file (never directories).".into());
            }
            if !parts.is_empty() {
                out.push(format!("  - {}", parts.join(" ")));
            }
            continue;
        }
        if line.starts_with("  - `edit` requires") && !has("edit") {
            continue;
        }
        let mut l = line.to_string();
        if l.starts_with("- **Arguments:**") {
            let ex: Vec<&str> = ["read", "sh"].into_iter().filter(|n| has(n)).collect();
            let repl = match ex.as_slice() {
                ["read", "sh"] => "functions such as `read` or `sh`",
                ["read"] => "functions such as `read`",
                ["sh"] => "functions such as `sh`",
                _ => "functions",
            };
            l = l.replace("functions such as `read` or `sh`", repl);
        }
        if l.starts_with("- **Compilation") && !has("read") {
            l = l.replace(" or `read(\"a.txt\")`", "");
        }
        if l.starts_with("- **Output format:**") {
            if !has("inspect") {
                l = l.replace(" rendered with `inspect(value)`", "");
            }
            if !has("sh") {
                l = l.replace(" (including `sh` results)", "");
            }
        }
        if l.starts_with("- **Uncapped output:**") {
            if !has("sh") {
                l = l.replace(", `sh().out`", "");
            }
            if !has("grep") {
                l = l.replace(" or `grep` with `limit`", "");
            }
        }
        if l.starts_with("- **120-second deadline:**") && !has("spawn") {
            l = l.replace(
                " Start long jobs with `spawn(fn, name)` (background tasks have no deadline).",
                "",
            );
        }
        if l.starts_with("- **Cooperative scheduler:**") {
            let waits: Vec<String> = ["sh", "sleep", "pick", "ask", "reload", "llm"]
                .into_iter()
                .filter(|n| has(n))
                .map(|n| format!("`{n}`"))
                .collect();
            let tail = match (has("sh"), has("sleep")) {
                (true, true) => {
                    "; `sh` and `sleep` also yield through user-created `coroutine.resume` and `coroutine.wrap`."
                }
                (true, false) => {
                    "; `sh` also yields through user-created `coroutine.resume` and `coroutine.wrap`."
                }
                (false, true) => {
                    "; `sleep` also yields through user-created `coroutine.resume` and `coroutine.wrap`."
                }
                (false, false) => ".",
            };
            if waits.is_empty() {
                l = "- **Cooperative scheduler:** Agent tool calls, hooks, custom slash commands, `open` handlers, and background tasks all run as coroutines in the same VM.".into();
            } else {
                l = format!(
                    "- **Cooperative scheduler:** Agent tool calls, hooks, custom slash commands, `open` handlers, and background tasks all run as coroutines in the same VM. Waiting calls ({}) yield so other coroutines continue running{tail}",
                    waits.join(", ")
                );
            }
        }
        if l.starts_with("- **Paused deadlines:**") {
            let paused: Vec<String> = ["pick", "ask", "reload"]
                .into_iter()
                .filter(|n| has(n))
                .map(|n| format!("`{n}`"))
                .collect();
            let mut s = String::new();
            if !paused.is_empty() {
                s.push_str(&format!(
                    "- **Paused deadlines:** {} pause the calling coroutine's remaining deadline while waiting and resume the remaining budget afterward.",
                    paused.join(", ")
                ));
            }
            if has("llm") {
                if s.is_empty() {
                    s.push_str("- **Paused deadlines:** `llm()` does not pause the deadline.");
                } else {
                    s.push_str(" `llm()` does not pause the deadline.");
                }
            }
            if s.is_empty() {
                continue;
            }
            l = s;
        }
        if l.starts_with("- **Non-yieldable boundaries:**") {
            let blocking: Vec<String> = ["sh", "sleep"]
                .into_iter()
                .filter(|n| has(n))
                .map(|n| format!("`{n}`"))
                .collect();
            let erring: Vec<String> = ["pick", "ask", "reload", "llm"]
                .into_iter()
                .filter(|n| has(n))
                .map(|n| format!("`{n}`"))
                .collect();
            let mut parts = vec![];
            if !blocking.is_empty() {
                parts.push(format!(
                    "{} block the VM thread until finished (still respecting cancellation and deadlines)",
                    blocking.join(" and ")
                ));
            }
            if !erring.is_empty() {
                parts.push(format!(
                    "{} raise `interactive wait cannot run here`",
                    erring.join(", ")
                ));
            }
            if parts.is_empty() {
                continue;
            }
            l = format!(
                "- **Non-yieldable boundaries:** Inside C callbacks where Lua cannot yield (for example a `string.gsub` function), {}.",
                parts.join(", while ")
            );
        }
        if l.starts_with("- **Loaded libraries:**") {
            let rx = match (has("grep"), has("sh")) {
                (true, true) => {
                    " Lua patterns are not regular expressions; use `grep` or `sh` for regexes."
                }
                (true, false) => {
                    " Lua patterns are not regular expressions; use `grep` for regexes."
                }
                (false, true) => " Lua patterns are not regular expressions; use `sh` for regexes.",
                (false, false) => "",
            };
            l = l.replace(
                " Lua patterns are not regular expressions; use `grep` or `sh` for regexes.",
                rx,
            );
        }
        if l.starts_with("- **Added globals:**") {
            match (has("json"), has("inspect")) {
                (true, true) => {}
                (true, false) => {
                    l = "- **Added globals:** `json.encode(value)` and `json.decode(text)` (maps JSON `null` to `nil`).".into();
                }
                (false, true) => {
                    l = "- **Added globals:** `inspect(value)`.".into();
                }
                (false, false) => continue,
            }
        }
        out.push(l);
    }
    out.join("\n")
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

        let f = docs_for(Some("functions"), &["sh", "delete"]);
        assert!(!f.contains("`sh(") && !f.contains("`sh`"), "{f}");
        assert!(!f.contains("`delete(") && !f.contains("`delete`"), "{f}");
        assert!(f.contains("`read(") && f.contains("`write("));

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
