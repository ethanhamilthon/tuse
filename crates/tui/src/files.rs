use std::path::Path;

/// Files and folders under `root` (folders end with `/`), skipping only `.git`.
pub fn list(root: &Path, max: usize) -> Vec<String> {
    let mut out = vec![];
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else {
            continue;
        };
        let mut entries: Vec<_> = rd.flatten().collect();
        entries.sort_by_key(|e| e.file_name());
        let mut subdirs = vec![];
        for e in entries {
            if out.len() >= max {
                return out;
            }
            let name = e.file_name();
            if name == ".git" {
                continue;
            }
            let path = e.path();
            let rel = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .display()
                .to_string();
            let is_dir = e.file_type().map(|t| t.is_dir()).unwrap_or(false);
            if is_dir {
                out.push(format!("{rel}/"));
                subdirs.push(path);
            } else {
                out.push(rel);
            }
        }
        stack.extend(subdirs.into_iter().rev());
    }
    out
}
