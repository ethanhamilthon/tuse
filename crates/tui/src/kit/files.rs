use std::path::Path;

pub fn list(root: &Path, dir: &str, max: usize) -> Vec<String> {
    let Ok(rd) = std::fs::read_dir(tuse_config::resolve_path(dir, root)) else {
        return vec![];
    };
    let mut entries: Vec<_> = rd.flatten().filter(|e| e.file_name() != ".git").collect();
    entries.sort_by_cached_key(|e| {
        let time = e
            .metadata()
            .ok()
            .and_then(|m| m.modified().or_else(|_| m.created()).ok());
        (std::cmp::Reverse(time), e.file_name())
    });
    entries
        .into_iter()
        .take(max)
        .map(|e| {
            let is_dir = e.file_type().is_ok_and(|t| t.is_dir());
            let name = e.file_name().to_string_lossy().into_owned();
            format!("{dir}{name}{}", if is_dir { "/" } else { "" })
        })
        .collect()
}

pub fn path(value: &str) -> std::path::PathBuf {
    if value.starts_with("~/") {
        tuse_config::resolve_path(value, Path::new(""))
    } else {
        value.into()
    }
}

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
        assert_eq!(
            path("~/a"),
            std::env::home_dir().unwrap_or_default().join("a")
        );
    }

    #[test]
    fn lists_newest_entries_before_the_cap_with_their_absolute_prefix() {
        let dir = std::env::temp_dir().join(format!("tuse-files-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        std::fs::create_dir_all(dir.join(".git")).unwrap();
        for (name, age) in [("a.txt", 600), ("z.txt", 60)] {
            let file = std::fs::File::create(dir.join(name)).unwrap();
            file.set_modified(std::time::SystemTime::now() - std::time::Duration::from_secs(age))
                .unwrap();
        }
        let abs = format!("{}/", dir.display());
        let got = list(Path::new("/nowhere"), &abs, 2);
        assert_eq!(got, [format!("{abs}sub/"), format!("{abs}z.txt")]);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
