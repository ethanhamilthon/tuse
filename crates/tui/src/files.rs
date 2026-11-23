use std::path::Path;

/// Entries of `root/dir` as paths relative to `root` (folders end with `/`),
/// skipping `.git`. `dir` is empty or ends with `/`; an absolute or `~/` `dir`
/// is listed as is, and the entries keep it as their prefix.
pub fn list(root: &Path, dir: &str, max: usize) -> Vec<String> {
    let Ok(rd) = std::fs::read_dir(tuse_config::resolve_path(dir, root)) else {
        return vec![];
    };
    let mut entries: Vec<_> = rd.flatten().collect();
    entries.sort_by_key(|e| e.file_name());
    entries
        .into_iter()
        .filter(|e| e.file_name() != ".git")
        .take(max)
        .map(|e| {
            let is_dir = e.file_type().is_ok_and(|t| t.is_dir());
            let name = e.file_name().to_string_lossy().into_owned();
            format!("{dir}{name}{}", if is_dir { "/" } else { "" })
        })
        .collect()
}

/// The file a picked `@` value points at: `~/x` is put under the home folder,
/// other values stay as they are (relative ones are read from the working directory).
pub fn path(value: &str) -> std::path::PathBuf {
    if value.starts_with("~/") {
        tuse_config::resolve_path(value, Path::new(""))
    } else {
        value.into()
    }
}

/// Splits an `@` query into the directory to list and its filename filter.
pub fn split(query: &str) -> (&str, &str) {
    if query == "~" {
        return ("~/", "");
    }
    query
        .rfind('/')
        .map_or(("", query), |i| query.split_at(i + 1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_query() {
        assert_eq!(split("some/"), ("some/", ""));
        assert_eq!(split("a/b/ma"), ("a/b/", "ma"));
        assert_eq!(split("src/ma"), ("src/", "ma"));
    }

    #[test]
    fn split_absolute_query() {
        assert_eq!(split("/"), ("/", ""));
        assert_eq!(split("/etc/ho"), ("/etc/", "ho"));
        assert_eq!(split("/etc/"), ("/etc/", ""));
        assert_eq!(split("~"), ("~/", ""));
        assert_eq!(split("~/do"), ("~/", "do"));
        assert_eq!(split("~/a/b"), ("~/a/", "b"));
    }

    #[test]
    fn picked_paths() {
        assert_eq!(path("src/a.rs"), Path::new("src/a.rs"));
        assert_eq!(path("/etc/hosts"), Path::new("/etc/hosts"));
        assert_eq!(path("~/a"), tuse_config::home().join("a"));
    }

    #[test]
    fn lists_absolute_dirs_with_their_prefix() {
        let dir = std::env::temp_dir().join(format!("tuse-files-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        std::fs::write(dir.join("a.txt"), "a").unwrap();
        let abs = format!("{}/", dir.display());
        let got = list(Path::new("/nowhere"), &abs, 10);
        assert_eq!(got, [format!("{abs}a.txt"), format!("{abs}sub/")]);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
