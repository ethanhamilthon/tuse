/// Built-in documentation topics read via `docs(topic=None)`.
const TOPICS: &[(&str, &str, &str)] = &[
    (
        "README.md",
        "Overview, install, usage, hotkeys, config, benchmark (long; read in slices)",
        include_str!("../../../README.md"),
    ),
    (
        "tool",
        "Python REPL tool, limits, timeout, imports",
        include_str!("../../../docs/tool.md"),
    ),
    (
        "functions",
        "Built-in Python functions (read, write, edit, grep, sh, task_*, docs)",
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
        "Config merging, expansion, keys, compaction and files",
        include_str!("../../../docs/config.md"),
    ),
    (
        "prompts",
        "#prompts, @files and [Image #N] in a message: the hidden context blocks",
        include_str!("../../../docs/prompts.md"),
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

fn topic_list() -> String {
    let mut out = String::from("Available docs topics:\n");
    for (name, summary, _) in TOPICS {
        out.push_str(&format!("- {name} — {summary}\n"));
    }
    out.push_str("\nCall docs(\"<topic>\") to read a section.");
    out
}

/// Returns the topic index when `topic` is `None`/empty, the topic markdown when found,
/// or an error message listing available topics when unknown.
pub fn docs(topic: Option<&str>) -> String {
    let Some(raw) = topic.map(str::trim).filter(|s| !s.is_empty()) else {
        return topic_list();
    };
    let key = raw.to_ascii_lowercase();
    let key = key.strip_suffix(".md").unwrap_or(&key);
    if let Some((_, _, body)) = TOPICS
        .iter()
        .find(|(name, _, _)| name.trim_end_matches(".md").eq_ignore_ascii_case(key))
    {
        return body.trim_end().to_string();
    }
    format!("Unknown docs topic \"{raw}\".\n\n{}", topic_list())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn docs_lists_finds_and_rejects_topics() {
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
    }
}
