include!(concat!(env!("OUT_DIR"), "/topics.rs"));

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
    crate::prompt::functions_doc(disabled)
}

fn render_tool(disabled: &[&str]) -> String {
    crate::prompt::tool_doc(disabled)
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
        for topic in ["plugins", "plugin-authoring", "subs"] {
            assert!(!index.contains(&format!("- {topic} —")));
            assert!(docs(Some(topic)).starts_with("Unknown docs topic"));
        }
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
