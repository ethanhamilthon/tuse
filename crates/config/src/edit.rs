use std::path::Path;

/// Sets a top-level `key: value` in a YAML file and keeps the rest of the file
/// as it is. A missing file is created; a missing key is appended.
pub fn set_key(path: &Path, key: &str, value: &str) -> Result<(), String> {
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(format!("{}: {e}", path.display())),
    };
    let new = format!("{key}: {}", scalar(value));
    let prefix = format!("{key}:");
    let mut out: Vec<&str> = vec![];
    let mut found = false;
    let mut skipping = false;
    for line in text.lines() {
        if !found && line.starts_with(&prefix) {
            found = true;
            skipping = true;
            out.push(&new);
            continue;
        }
        // The old value may be a block; drop its lines.
        if skipping && (line.starts_with([' ', '\t']) || line.starts_with("- ")) {
            continue;
        }
        skipping = false;
        out.push(line);
    }
    if !found {
        out.push(&new);
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    std::fs::write(path, out.join("\n") + "\n").map_err(|e| format!("{}: {e}", path.display()))
}

fn scalar(v: &str) -> String {
    let plain = v.starts_with(|c: char| c.is_ascii_alphabetic())
        && v.chars()
            .all(|c| c.is_ascii_alphanumeric() || "._/-".contains(c));
    if plain {
        v.to_string()
    } else {
        format!("{v:?}")
    }
}
