//! Models sometimes call a function of `python` (`ask`, `grep`...) as a tool of its own;
//! such a call runs as `python` code instead of failing with "unknown tool".
use serde_json::Value;

const BUILTINS: &[&str] = &[
    "ask",
    "draw_ui",
    "read",
    "write",
    "edit",
    "grep",
    "sh",
    "docs",
    "task_run",
    "task_add",
    "task_get",
    "task_stop",
    "task_list",
];

/// Arguments that take a list; a model may send the list as a JSON string.
const LISTS: &[&str] = &["options"];

pub fn is_builtin(name: &str) -> bool {
    BUILTINS.contains(&name)
}

/// The code `name(**args)` for a built-in function, or `None` for other names.
pub fn code(name: &str, args: &Value) -> Option<String> {
    if !is_builtin(name) {
        return None;
    }
    let mut parts = vec![];
    if let Some(map) = args.as_object() {
        for (k, v) in map {
            if k == "title" || !is_ident(k) {
                continue;
            }
            let v = match v {
                Value::String(s) if LISTS.contains(&k.as_str()) => {
                    serde_json::from_str::<Value>(s).unwrap_or_else(|_| v.clone())
                }
                _ => v.clone(),
            };
            parts.push(format!("{k}={}", literal(&v)));
        }
    }
    Some(format!("{name}({})", parts.join(", ")))
}

fn is_ident(s: &str) -> bool {
    s.chars()
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// A JSON value as a Python literal.
fn literal(v: &Value) -> String {
    match v {
        Value::Null => "None".into(),
        Value::Bool(b) => if *b { "True" } else { "False" }.into(),
        Value::Number(n) => n.to_string(),
        // A JSON string is a valid Python string literal.
        Value::String(_) => v.to_string(),
        Value::Array(a) => format!("[{}]", a.iter().map(literal).collect::<Vec<_>>().join(", ")),
        Value::Object(o) => format!(
            "{{{}}}",
            o.iter()
                .map(|(k, v)| format!("{}: {}", Value::String(k.clone()), literal(v)))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn builds_calls_for_builtins_only() {
        let c = code(
            "ask",
            &json!({"question": "Go \"on\"?", "options": "[\"yes\", \"no\"]", "title": "t"}),
        );
        assert_eq!(
            c.unwrap(),
            r#"ask(options=["yes", "no"], question="Go \"on\"?")"#
        );
        let c = code(
            "grep",
            &json!({"pattern": "x", "ignore_case": true, "limit": 3, "glob": null}),
        );
        assert_eq!(
            c.unwrap(),
            r#"grep(glob=None, ignore_case=True, limit=3, pattern="x")"#
        );
        assert_eq!(code("task_list", &json!({})).unwrap(), "task_list()");
        assert!(code("browse", &json!({})).is_none());
    }
}
