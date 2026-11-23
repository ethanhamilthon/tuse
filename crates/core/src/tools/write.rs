use super::{diff, fail, fs, io_fail};
use std::path::Path;

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

pub fn append(path: &Path, text: &str) -> Result<(String, String), String> {
    if path.is_dir() {
        return Err(fail(format!("{} is a directory", path.display())));
    }
    let before = std::fs::read(path)
        .ok()
        .map(|b| String::from_utf8_lossy(&b).into_owned());
    match before {
        None => {
            fs::write_atomic(path, text.as_bytes()).map_err(|e| io_fail(path, &e))?;
            let now = text.lines().count();
            Ok((
                format!("created {} ({now} lines)", path.display()),
                diff::whole(text),
            ))
        }
        Some(old) => {
            let mut combined = String::with_capacity(old.len() + text.len());
            combined.push_str(&old);
            combined.push_str(text);
            fs::write_atomic(path, combined.as_bytes()).map_err(|e| io_fail(path, &e))?;
            Ok((
                format!(
                    "appended {} ({} -> {} lines)",
                    path.display(),
                    old.lines().count(),
                    combined.lines().count()
                ),
                diff::changed(&old, &combined),
            ))
        }
    }
}

pub fn delete(path: &Path) -> Result<(String, String), String> {
    let meta = std::fs::symlink_metadata(path).map_err(|e| io_fail(path, &e))?;
    if meta.is_dir() {
        return Err(fail(format!(
            "{} is a directory; delete() only removes a single file",
            path.display()
        )));
    }
    let (msg, diff) = match std::fs::read(path) {
        Ok(bytes) if bytes.iter().take(8192).any(|&b| b == 0) => (
            format!("deleted {} ({} bytes)", path.display(), bytes.len()),
            format!("- [binary file, {} bytes]", bytes.len()),
        ),
        Ok(bytes) => {
            let old = String::from_utf8_lossy(&bytes);
            (
                format!("deleted {} ({} lines)", path.display(), old.lines().count()),
                diff::removed(&old),
            )
        }
        Err(_) => (format!("deleted {}", path.display()), String::new()),
    };
    std::fs::remove_file(path).map_err(|e| io_fail(path, &e))?;
    Ok((msg, diff))
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

        let (msg, d) = append(&file, "w\n").unwrap();
        assert!(
            msg.contains("1 -> 2 lines") && d.contains("+    2 | w"),
            "{msg}\n{d}"
        );
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "z\nw\n");

        let (msg, d) = delete(&file).unwrap();
        assert!(msg.starts_with("deleted") && d.contains("-    1 | z") && d.contains("-    2 | w"));
        assert!(!file.exists());
        assert!(
            delete(&dir.join("a")).is_err(),
            "must not delete directories"
        );
        std::fs::remove_dir_all(dir).ok();
    }
}
