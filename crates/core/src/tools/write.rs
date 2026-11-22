use super::{diff, fail, fs, io_fail};
use std::path::Path;

/// Writes `content`; returns the confirmation and the diff of the change.
pub fn run(path: &Path, content: &str) -> Result<(String, String), String> {
    if path.is_dir() {
        return Err(fail(format!("{} is a directory", path.display())));
    }
    let before = std::fs::read(path)
        .ok()
        .map(|b| String::from_utf8_lossy(&b).into_owned());
    fs::write_atomic(path, content.as_bytes()).map_err(|e| io_fail(path, &e))?;
    let now = content.lines().count();
    Ok(match before {
        None => (
            format!("created {} ({now} lines)", path.display()),
            diff::whole(content),
        ),
        Some(old) => (
            format!(
                "overwrote {} ({} -> {now} lines)",
                path.display(),
                old.lines().count()
            ),
            diff::changed(&old, content),
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_parents_and_overwrites() {
        let dir = std::env::temp_dir().join(format!("tuse-write-{}", std::process::id()));
        let file = dir.join("a/b/c.txt");
        let (msg, diff) = run(&file, "x\ny\n").unwrap();
        assert!(msg.starts_with("created") && diff.contains("+    1 | x"));
        let (msg, diff) = run(&file, "z\n").unwrap();
        assert!(msg.contains("2 -> 1 lines"), "{msg}");
        assert!(
            diff.contains("-    1 | x") && diff.contains("+    1 | z"),
            "{diff}"
        );
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "z\n");
        assert!(run(&dir.join("a"), "").is_err());
        std::fs::remove_dir_all(dir).ok();
    }
}
