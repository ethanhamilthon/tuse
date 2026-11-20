/// Built-in documentation topics read via `docs(topic=None)`.
const TOPICS: &[(&str, &str, &str)] = &[
    (
        "tool",
        "Python REPL tool, limits, timeout, imports",
        include_str!("../../../docs/tool.md"),
    ),
    (
        "functions",
        "Built-in Python functions (read, write, edit, grep, sh, ask, show, task_*, docs)",
        include_str!("../../../docs/functions.md"),
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
        "hooks",
        "Event hooks and prelude",
        include_str!("../../../docs/hooks.md"),
    ),
    (
        "theme",
        "10-color themes and settings.ui",
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
    if let Some((_, _, body)) = TOPICS
        .iter()
        .find(|(name, _, _)| name.eq_ignore_ascii_case(raw))
    {
        return body.trim_end().to_string();
    }
    format!("Unknown docs topic \"{raw}\".\n\n{}", topic_list())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn docs_index_lists_all_topics() {
        let index = docs(None);
        for (name, _, _) in TOPICS {
            assert!(index.contains(&format!("- {name} —")), "missing {name}");
        }
        assert_eq!(index, docs(Some("  ")));
    }

    #[test]
    fn docs_topic_lookup_and_unknown() {
        let hooks = docs(Some("hooks"));
        assert!(hooks.starts_with("# Hooks"));
        assert_eq!(hooks, docs(Some(" HOOKS ")));

        let err = docs(Some("nope"));
        assert!(err.starts_with("Unknown docs topic \"nope\"."));
        assert!(err.contains("- config —"));
    }
}
