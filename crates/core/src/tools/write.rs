use super::{Output, fs, text_arg};
use serde_json::Value;
use std::path::Path;

pub fn run(path: &Path, args: &Value) -> Output {
    let content = match text_arg(args, "content") {
        Ok(c) => c,
        Err(e) => return e,
    };
    if path.is_dir() {
        return Output::err(format!("{} is a directory", path.display()));
    }
    let before = std::fs::read(path)
        .ok()
        .map(|b| lines(&String::from_utf8_lossy(&b)));
    if let Err(e) = fs::write_atomic(path, content.as_bytes()) {
        return Output::err(format!("{}: {e}", path.display()));
    }
    let now = lines(content);
    Output::ok(match before {
        None => format!("created {} ({now} lines)", path.display()),
        Some(was) => format!("overwrote {} ({was} -> {now} lines)", path.display()),
    })
}

fn lines(s: &str) -> usize {
    s.lines().count()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_parents_and_overwrites() {
        let dir = std::env::temp_dir().join(format!("tuse-write-{}", std::process::id()));
        let file = dir.join("a/b/c.txt");
        let args = |c: &str| serde_json::json!({"content": c});
        assert!(run(&file, &args("x\ny\n")).text.starts_with("created"));
        let o = run(&file, &args("z\n"));
        assert!(o.text.contains("2 -> 1 lines"), "{}", o.text);
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "z\n");
        assert!(!run(&dir.join("a"), &args("")).ok);
        std::fs::remove_dir_all(dir).ok();
    }
}
