use super::{Output, diff, fs, text_arg};
use serde_json::Value;
use std::path::Path;

pub fn run(path: &Path, args: &Value) -> Output {
    let (old, new) = match (text_arg(args, "old"), text_arg(args, "new")) {
        (Ok(o), Ok(n)) => (o, n),
        (Err(e), _) | (_, Err(e)) => return e,
    };
    let all = args["replace_all"].as_bool().unwrap_or(false);
    if old.is_empty() {
        return Output::err("`old` is empty; use the write tool to create or fill a file");
    }
    if old == new {
        return Output::err("`old` and `new` are the same");
    }
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) => return Output::err(format!("{}: {e}", path.display())),
    };
    let Ok(text) = String::from_utf8(bytes) else {
        return Output::err(format!("{} is not UTF-8 text", path.display()));
    };
    // A CRLF file matches `\n` in `old` and keeps its CRLF.
    let crlf = text.contains("\r\n") && !old.contains('\r');
    let (old, new) = if crlf {
        (old.replace('\n', "\r\n"), new.replace('\n', "\r\n"))
    } else {
        (old.to_string(), new.to_string())
    };
    let at: Vec<(usize, usize)> = text
        .match_indices(&old)
        .map(|(i, m)| (i, m.len()))
        .collect();
    if at.is_empty() {
        return Output::err(format!("`old` was not found in {}", path.display()));
    }
    if at.len() > 1 && !all {
        let lines: Vec<String> = diff::hunks(&text, &at, "")
            .iter()
            .take(10)
            .map(|h| h.start.to_string())
            .collect();
        return Output::err(format!(
            "`old` was found {} times (lines {}); add context to make it unique or pass replace_all",
            at.len(),
            lines.join(", ")
        ));
    }
    let hunks = diff::hunks(&text, &at, &new);
    let result = text.replace(&old, &new);
    if let Err(e) = fs::write_atomic(path, result.as_bytes()) {
        return Output::err(format!("{}: {e}", path.display()));
    }
    let n = at.len();
    let head = format!(
        "edited {} ({n} replacement{})",
        path.display(),
        if n == 1 { "" } else { "s" }
    );
    Output::ok(format!("{head}\n{}", diff::render(&hunks)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn replaces_once_refuses_ambiguity_and_keeps_crlf() {
        let dir = std::env::temp_dir().join(format!("tuse-edit-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("a.txt");
        std::fs::write(&f, "one\ntwo\nthree\ntwo\n").unwrap();
        let o = run(&f, &json!({"old": "one", "new": "1"}));
        assert!(
            o.ok && o.text.contains("-    1 | one") && o.text.contains("+    1 | 1"),
            "{}",
            o.text
        );
        let o = run(&f, &json!({"old": "two", "new": "2"}));
        assert!(
            !o.ok && o.text.contains("2 times (lines 2, 4)"),
            "{}",
            o.text
        );
        assert!(run(&f, &json!({"old": "two", "new": "2", "replace_all": true})).ok);
        assert_eq!(std::fs::read_to_string(&f).unwrap(), "1\n2\nthree\n2\n");
        assert!(!run(&f, &json!({"old": "zzz", "new": "y"})).ok);
        std::fs::write(&f, "a\r\nb\r\nc\r\n").unwrap();
        assert!(run(&f, &json!({"old": "a\nb", "new": "x\ny"})).ok);
        assert_eq!(std::fs::read_to_string(&f).unwrap(), "x\r\ny\r\nc\r\n");
        std::fs::remove_dir_all(dir).ok();
    }
}
