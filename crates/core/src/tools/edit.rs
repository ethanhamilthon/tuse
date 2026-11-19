use super::{diff, fs, io_fail};
use std::path::Path;
use tuse_py::Fail;

/// Replaces `old` with `new`; returns the confirmation and the diff.
pub fn run(path: &Path, old: &str, new: &str, all: bool) -> Result<(String, String), Fail> {
    if old.is_empty() {
        return Err(Fail::value(
            "`old` is empty; use write() to create or fill a file",
        ));
    }
    if old == new {
        return Err(Fail::value("`old` and `new` are the same"));
    }
    let bytes = std::fs::read(path).map_err(|e| io_fail(path, &e))?;
    let Ok(text) = String::from_utf8(bytes) else {
        return Err(Fail::value(format!("{} is not UTF-8 text", path.display())));
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
        return Err(Fail::value(format!(
            "`old` was not found in {}",
            path.display()
        )));
    }
    if at.len() > 1 && !all {
        let lines: Vec<String> = diff::hunks(&text, &at, "")
            .iter()
            .take(10)
            .map(|h| h.start.to_string())
            .collect();
        return Err(Fail::value(format!(
            "`old` was found {} times (lines {}); add context to make it unique or pass replace_all=True",
            at.len(),
            lines.join(", ")
        )));
    }
    let hunks = diff::hunks(&text, &at, &new);
    let result = text.replace(&old, &new);
    fs::write_atomic(path, result.as_bytes()).map_err(|e| io_fail(path, &e))?;
    let n = at.len();
    let head = format!(
        "edited {} ({n} replacement{})",
        path.display(),
        if n == 1 { "" } else { "s" }
    );
    Ok((head, diff::render(&hunks)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replaces_once_refuses_ambiguity_and_keeps_crlf() {
        let dir = std::env::temp_dir().join(format!("tuse-edit-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("a.txt");
        std::fs::write(&f, "one\ntwo\nthree\ntwo\n").unwrap();
        let (_, d) = run(&f, "one", "1", false).unwrap();
        assert!(
            d.contains("-    1 | one") && d.contains("+    1 | 1"),
            "{d}"
        );
        let e = run(&f, "two", "2", false).unwrap_err();
        assert!(e.1.contains("2 times (lines 2, 4)"), "{}", e.1);
        assert!(run(&f, "two", "2", true).is_ok());
        assert_eq!(std::fs::read_to_string(&f).unwrap(), "1\n2\nthree\n2\n");
        assert!(run(&f, "zzz", "y", false).is_err());
        std::fs::write(&f, "a\r\nb\r\nc\r\n").unwrap();
        assert!(run(&f, "a\nb", "x\ny", false).is_ok());
        assert_eq!(std::fs::read_to_string(&f).unwrap(), "x\r\ny\r\nc\r\n");
        std::fs::remove_dir_all(dir).ok();
    }
}
