use std::path::Path;

/// Entries of `root/dir` as paths relative to `root` (folders end with `/`),
/// skipping `.git`. `dir` is empty or ends with `/`.
pub fn list(root: &Path, dir: &str, max: usize) -> Vec<String> {
    let Ok(rd) = std::fs::read_dir(root.join(dir)) else {
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

/// Splits an `@` query: `some/*x` is (`some/`, `x`); without `/*` the
/// directory is empty and the whole query is the filter.
pub fn split(query: &str) -> (&str, &str) {
    match query.rfind("/*") {
        Some(i) => (&query[..=i], &query[i + 2..]),
        None => ("", query),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_query() {
        assert_eq!(split("some/*"), ("some/", ""));
        assert_eq!(split("a/b/*ma"), ("a/b/", "ma"));
        assert_eq!(split("src/ma"), ("", "src/ma"));
    }
}
