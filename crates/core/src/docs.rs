include!(concat!(env!("OUT_DIR"), "/topics.rs"));

use crate::prompt::{AGENT_CONTEXT, agent_context};

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
        out.push_str(&format!("- {name} — {summary}\n"));
    }
    if !disabled.contains(&"docs") {
        out.push_str("\nCall docs(\"<topic>\") to read a section.");
    }
    out
}

fn render(name: &str, body: &str, disabled: &[&str]) -> String {
    let text = match (name, body.find(AGENT_CONTEXT)) {
        ("lua", Some(i)) if !disabled.is_empty() => {
            format!("{}{}", &body[..i], agent_context(disabled))
        }
        _ => body.to_string(),
    };
    let text = text.trim_end();
    if disabled.is_empty() {
        text.to_string()
    } else {
        format!("{}{text}", disabled_notice(disabled))
    }
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
    match TOPICS.iter().find(|(name, _, _)| *name == key) {
        Some((name, _, body)) => render(name, body, disabled),
        None => format!("Unknown docs topic \"{raw}\".\n\n{}", topic_list(disabled)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lua_page_ends_with_the_agent_context() {
        let page = docs(Some("lua"));
        let i = page
            .find(AGENT_CONTEXT)
            .expect("lua.md misses the agent context");
        let expected = agent_context(&[]);
        assert!(
            page[i..] == *expected.trim_end(),
            "docs/lua.md must end with:\n\n{expected}"
        );
    }

    #[test]
    fn docs_lists_finds_and_filters_disabled_builtins() {
        let index = docs(None);
        for (name, summary, _) in TOPICS {
            assert!(!summary.is_empty(), "{name} has no summary");
            assert!(index.contains(&format!("- {name} — ")), "missing {name}");
        }
        assert_eq!(TOPICS.len(), 7);
        assert_eq!(index, docs(Some("  ")));
        for topic in ["functions", "tool", "commands", "hooks", "nope"] {
            assert!(docs(Some(topic)).starts_with(&format!("Unknown docs topic \"{topic}\".")));
        }
        assert_eq!(docs(Some(" Config.MD ")), docs(Some("config")));
        assert_eq!(docs(Some("README.md")), docs(Some("readme")));
        assert!(docs(Some("json")).starts_with("# "));

        let lua = docs_for(Some("lua"), &["sh", "delete", "docs"]);
        assert!(lua.starts_with("Disabled in this session: delete, docs, sh."));
        let tail = &lua[lua.find(AGENT_CONTEXT).unwrap()..];
        assert!(!tail.contains("`sh(") && !tail.contains("sh(cmd"), "{tail}");
        assert!(
            !tail.contains("`delete(") && !tail.contains("`docs("),
            "{tail}"
        );
        assert!(tail.contains("`read("));
        assert!(!docs_for(None, &["docs"]).contains("Call docs("));
    }
}
