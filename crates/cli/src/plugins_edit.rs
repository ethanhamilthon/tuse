//! Adds and removes `plugins` entries in the text of a config file.

/// The entry as Lua: `"name"` or `{ "name", ref = "r" }`.
pub fn entry(name: &str, git_ref: Option<&str>) -> String {
    match git_ref {
        Some(r) => format!("{{ {name:?}, ref = {r:?} }}"),
        None => format!("{name:?}"),
    }
}

fn indent_of(line: &str) -> &str {
    &line[..line.len() - line.trim_start().len()]
}

/// Inserts `item` into `plugins = {` or, failing that, into `return {`.
/// `None` when the file has neither on a line of its own.
pub fn add(text: &str, item: &str) -> Option<String> {
    let lines: Vec<&str> = text.split_inclusive('\n').collect();
    let find = |pat: &str| {
        lines
            .iter()
            .position(|l| l.trim_start().starts_with(pat) && l.trim_end().ends_with('{'))
    };
    let (at, inner) = if let Some(i) = find("plugins = {").or_else(|| find("plugins={")) {
        (i, item.to_string())
    } else {
        let i = find("return {")?;
        let pad = format!("{}    ", indent_of(lines[i]));
        (i, format!("plugins = {{\n{pad}    {item},\n{pad}}}"))
    };
    let pad = match lines.get(at + 1) {
        Some(next) if !next.trim().is_empty() && !next.trim_start().starts_with('}') => {
            indent_of(next).to_string()
        }
        _ => format!("{}    ", indent_of(lines[at])),
    };
    let mut out: String = lines[..=at].concat();
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out.push_str(&format!("{pad}{inner},\n"));
    out.push_str(&lines[at + 1..].concat());
    Some(out)
}

/// Whether the file mentions the plugin as a quoted name.
pub fn has(text: &str, name: &str) -> bool {
    text.contains(&format!("{name:?}")) || text.contains(&format!("'{name}'"))
}

/// Drops the line holding the entry when the whole entry is on it.
pub fn remove(text: &str, name: &str) -> Option<String> {
    let quoted = [format!("{name:?}"), format!("'{name}'")];
    let mut found = false;
    let out: String = text
        .split_inclusive('\n')
        .filter(|l| {
            let t = l.trim().trim_end_matches(',');
            let hit = quoted.iter().any(|q| {
                t == q
                    || (t.starts_with('{')
                        && t.ends_with('}')
                        && t[1..].trim_start().starts_with(q.as_str()))
            });
            found |= hit;
            !hit
        })
        .collect();
    found.then_some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edits_the_list() {
        let t = "return {\n    settings = {},\n}\n";
        let a = add(t, &entry("a/b", None)).unwrap();
        assert_eq!(
            a,
            "return {\n    plugins = {\n        \"a/b\",\n    },\n    settings = {},\n}\n"
        );
        let b = add(&a, &entry("c/d", Some("v1"))).unwrap();
        assert!(
            b.contains("    plugins = {\n        { \"c/d\", ref = \"v1\" },\n        \"a/b\",\n")
        );
        assert!(has(&b, "c/d") && !has(&b, "c"));
        assert_eq!(
            remove(&remove(&b, "c/d").unwrap(), "a/b").unwrap(),
            "return {\n    plugins = {\n    },\n    settings = {},\n}\n"
        );
        assert_eq!(add("return g\n", "\"a/b\""), None);
        assert_eq!(remove(t, "a/b"), None);
    }
}
