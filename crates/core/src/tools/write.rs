use super::{diff, fail, io_fail, write_atomic};
use std::borrow::Cow;
use std::path::Path;

pub(super) fn run(path: &Path, text: &str, append: bool) -> Result<(String, String), String> {
    if path.is_dir() {
        return Err(format!("{} is a directory", path.display()));
    }
    let before = std::fs::read(path)
        .ok()
        .map(|b| String::from_utf8_lossy(&b).into_owned());
    let content = match &before {
        Some(old) if append => Cow::Owned([old.as_str(), text].concat()),
        _ => Cow::Borrowed(text),
    };
    write_atomic(path, content.as_bytes()).map_err(|e| io_fail(path, &e))?;
    let now = content.lines().count();
    let name = path.display();
    Ok(match before {
        None => (
            format!("created {name} ({now} lines)"),
            diff::whole(&content),
        ),
        Some(old) => {
            let verb = if append { "appended" } else { "overwrote" };
            (
                format!("{verb} {name} ({} -> {now} lines)", old.lines().count()),
                diff::changed(&old, &content),
            )
        }
    })
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
    use super::super::{append, delete, write};

    #[test]
    fn creates_parents_and_overwrites() {
        let dir = std::env::temp_dir().join(format!("tuse-write-{}", std::process::id()));
        let file = dir.join("a/b/c.txt");
        let (msg, diff) = write(&file, "x\ny\n").unwrap();
        assert!(msg.starts_with("created") && diff.contains("+    1 | x"));
        let (msg, diff) = write(&file, "z\n").unwrap();
        assert!(msg.contains("2 -> 1 lines"), "{msg}");
        assert!(
            diff.contains("-    1 | x") && diff.contains("+    1 | z"),
            "{diff}"
        );
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "z\n");
        assert!(write(&dir.join("a"), "").is_err());

        let (msg, d) = append(&file, "w\n").unwrap();
        assert!(
            msg.contains("1 -> 2 lines") && d.contains("+    2 | w"),
            "{msg}\n{d}"
        );
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "z\nw\n");

        let (msg, d) = delete(&file).unwrap();
        assert!(msg.starts_with("deleted") && d.contains("-    1 | z") && d.contains("-    2 | w"));
        assert!(!file.exists());
        let (msg, d) = append(&file, "new\n").unwrap();
        assert!(msg.starts_with("created") && d.contains("+    1 | new"));
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "new\n");
        assert!(
            delete(&dir.join("a")).is_err(),
            "must not delete directories"
        );
        std::fs::remove_dir_all(dir).ok();
    }
}
