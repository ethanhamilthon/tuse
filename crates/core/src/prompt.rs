use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::sync::OnceLock;
use tuse_api::ToolDef;
use tuse_config::Config;

const PAGE: &str = include_str!("../../../docs/system-prompt.md");
const DOCS_ONLY: &str = " <!-- docs-only -->";

struct Page {
    blocks: BTreeMap<&'static str, &'static str>,
    functions: Vec<(&'static str, &'static str, &'static str, bool)>,
}

fn page() -> &'static Page {
    static CELL: OnceLock<Page> = OnceLock::new();
    CELL.get_or_init(|| parse(PAGE))
}

fn parse(text: &'static str) -> Page {
    let mut blocks = BTreeMap::new();
    let mut functions = vec![];
    let mut in_functions = false;
    let mut group = "";
    let mut lines = text.lines().peekable();
    while let Some(line) = lines.next() {
        if line == "## Functions" {
            in_functions = true;
            continue;
        }
        if line.starts_with("## ") {
            in_functions = false;
        }
        let Some(name) = line.strip_prefix("### ") else {
            if in_functions && line.starts_with("- `") {
                let (body, docs_only) = match line.strip_suffix(DOCS_ONLY) {
                    Some(b) => (b, true),
                    None => (line, false),
                };
                let id = body[3..].split('`').next().unwrap_or("");
                let id = id.split(['(', '.']).next().unwrap_or(id);
                functions.push((group, id, body, docs_only));
            }
            continue;
        };
        if in_functions {
            group = name;
            continue;
        }
        if lines.peek() == Some(&"") {
            lines.next();
        }
        if lines.peek() == Some(&"```text") {
            lines.next();
            let start = text.len() - lines.clone().map(|l| l.len() + 1).sum::<usize>();
            let mut end = start;
            for l in lines.by_ref() {
                if l == "```" {
                    break;
                }
                end += l.len() + 1;
            }
            blocks.insert(name, text[start..end].trim_end_matches('\n'));
        }
    }
    Page { blocks, functions }
}

fn block(name: &str) -> &'static str {
    page().blocks.get(name).copied().unwrap_or("")
}

struct Enabled<'a>(&'a [&'a str]);

impl Enabled<'_> {
    fn has(&self, name: &str) -> bool {
        !self.0.contains(&name)
    }

    fn cond(&self, spec: &str) -> bool {
        if spec.contains('&') {
            spec.split('&').all(|n| self.has(n))
        } else {
            spec.split('|').any(|n| self.has(n))
        }
    }

    fn line(&self, name: &str) -> Option<&'static str> {
        page()
            .functions
            .iter()
            .find(|(_, id, _, _)| *id == name)
            .map(|(_, _, line, _)| *line)
    }
}

fn join_clauses(items: &[String]) -> String {
    match items {
        [] => String::new(),
        [a] => a.clone(),
        [a, b] => format!("{a} and {b}"),
        [rest @ .., last] => format!("{}, and {last}", rest.join(", ")),
    }
}

pub(crate) fn prefer_sentence(disabled: &[&str], with_parens: bool) -> String {
    let on = Enabled(disabled);
    let mut parts = vec![];
    for clause in block("prefer-clauses").lines() {
        let Some((names, template)) = clause.split_once(": ") else {
            continue;
        };
        let names: Vec<&str> = names.split(", ").filter(|n| on.has(n)).collect();
        if names.is_empty() {
            continue;
        }
        let fmt = |n: &str| {
            if with_parens {
                format!("{n}()")
            } else {
                format!("`{n}`")
            }
        };
        let list = names.iter().map(|n| fmt(n)).collect::<Vec<_>>().join("/");
        parts.push(template.replace("{fn}", &list));
    }
    if parts.is_empty() {
        String::new()
    } else {
        format!(
            " {}",
            block("prefer").replace("{clauses}", &join_clauses(&parts))
        )
    }
}

fn expand(template: &str, disabled: &[&str]) -> String {
    let on = Enabled(disabled);
    let mut out_lines: Vec<String> = vec![];
    for line in template.split('\n') {
        let had = line.contains("{{");
        let mut out = String::new();
        let mut rest = line;
        while let Some(i) = rest.find("{{") {
            out.push_str(&rest[..i]);
            let Some(j) = rest[i..].find("}}") else {
                out.push_str(&rest[i..]);
                rest = "";
                break;
            };
            let inner = &rest[i + 2..i + j];
            rest = &rest[i + j + 2..];
            out.push_str(&placeholder(inner, &on, disabled));
        }
        out.push_str(rest);
        if had && out.trim().is_empty() {
            continue;
        }
        out_lines.push(out);
    }
    out_lines.join("\n")
}

fn placeholder(inner: &str, on: &Enabled, disabled: &[&str]) -> String {
    if inner == "prefer" {
        return prefer_sentence(disabled, false);
    }
    if inner == "prefer()" {
        return prefer_sentence(disabled, true);
    }
    if let Some(list) = inner.strip_prefix("each:") {
        let names: Vec<String> = list
            .split(',')
            .filter(|n| on.has(n))
            .map(|n| format!("`{n}`"))
            .collect();
        return names.join(", ");
    }
    if let Some(list) = inner.strip_prefix("lines:") {
        let lines: Vec<&str> = list
            .split(',')
            .filter(|n| on.has(n))
            .filter_map(|n| on.line(n))
            .collect();
        return lines.join("\n");
    }
    match inner.split_once(':') {
        Some((spec, text)) if on.cond(spec) => text.to_string(),
        Some(_) | None => String::new(),
    }
}

pub(crate) fn function_sections(disabled: &[&str]) -> Vec<(&'static str, Vec<&'static str>)> {
    let on = Enabled(disabled);
    let mut out: Vec<(&'static str, Vec<&'static str>)> = vec![];
    for (group, id, line, _) in &page().functions {
        if !on.has(id) {
            continue;
        }
        match out.last_mut() {
            Some((g, lines)) if g == group => lines.push(line),
            _ => out.push((group, vec![line])),
        }
    }
    out
}

pub(crate) fn functions_doc(disabled: &[&str]) -> String {
    let mut out = block("functions-doc-header").to_string();
    let pref = expand(block("functions-doc-preference"), disabled);
    if !pref.is_empty() {
        out.push_str("\n\n");
        out.push_str(pref.trim_start());
    }
    for (group, lines) in function_sections(disabled) {
        out.push_str(&format!("\n\n## {group}\n"));
        for line in lines {
            out.push('\n');
            out.push_str(line);
        }
    }
    out
}

pub(crate) fn tool_doc(disabled: &[&str]) -> String {
    let mut out = expand(block("tool-doc"), disabled);
    let tail = expand(block("tool-doc-functions"), disabled);
    if !tail.is_empty() && !tail.ends_with("{{lines}}") && tail.lines().count() > 1 {
        out.push_str("\n\n");
        out.push_str(&tail);
    }
    out
}

pub fn system_prompt(config: &Config, compacted: bool) -> String {
    let disabled = config.disabled_builtins();
    let on = Enabled(&disabled);
    let mut base = expand(block("prefix"), &disabled);
    let bullets: Vec<&str> = page()
        .functions
        .iter()
        .filter(|(_, id, _, docs_only)| !docs_only && on.has(id))
        .map(|(_, _, line, _)| *line)
        .collect();
    if !bullets.is_empty() {
        base.push_str("\n\n");
        base.push_str(&expand(block("functions-header"), &disabled));
        for b in bullets {
            base.push('\n');
            base.push_str(b);
        }
    }
    let mut parts = vec![base];
    parts.extend(config.systems.iter().map(|s| s.trim_end().to_string()));
    let mut out = parts.join("\n\n");
    if compacted {
        out.push_str("\n\n");
        out.push_str(block("compacted"));
    }
    out
}

fn tool(name: &str, description: &str, properties: Value, required: &[&str]) -> ToolDef {
    ToolDef {
        name: name.to_string(),
        description: description.to_string(),
        parameters: json!({"type": "object", "properties": properties, "required": required}),
    }
}

pub fn tools(config: &Config) -> Vec<ToolDef> {
    let disabled = config.disabled_builtins();
    let desc = expand(block("tool-description"), &disabled);
    vec![tool(
        "lua",
        &desc,
        json!({
            "code": {"type": "string", "description": block("tool-code-parameter")}
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

        let desc = tools(&config)[0].description.clone();
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
