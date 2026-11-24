use super::fail;
use ignore::WalkBuilder;
use ignore::overrides::OverrideBuilder;
use regex::RegexBuilder;
use std::io::{BufRead, BufReader};
use std::path::Path;

const DEFAULT_LIMIT: usize = 100;
const LINE_CHARS: usize = 300;

pub struct Opts {
    pub paths: Vec<String>,
    pub glob: Option<String>,
    pub ignore_case: bool,
    pub literal: bool,
    pub context: usize,
    pub limit: Option<usize>,
}

pub fn run(
    pattern: &str,
    o: Opts,
    cwd: &Path,
    stopped: &dyn Fn() -> bool,
) -> Result<String, String> {
    let roots: Vec<_> = if o.paths.is_empty() {
        vec![tuse_config::resolve_path(".", cwd)]
    } else {
        o.paths
            .iter()
            .map(|p| tuse_config::resolve_path(p, cwd))
            .collect()
    };
    let (glob, ignore_case, context) = (o.glob, o.ignore_case, o.context);
    let limit = o.limit.unwrap_or(DEFAULT_LIMIT).max(1);
    let pat = if o.literal {
        std::borrow::Cow::Owned(regex::escape(pattern))
    } else {
        std::borrow::Cow::Borrowed(pattern)
    };
    let re = RegexBuilder::new(&pat)
        .case_insensitive(ignore_case)
        .build()
        .map_err(|e| fail(format!("bad pattern: {e}")))?;
    for root in &roots {
        if !root.exists() {
            return Err(fail(format!("{}: no such file or folder", root.display())));
        }
    }
    let mut walk = WalkBuilder::new(&roots[0]);
    for root in &roots[1..] {
        walk.add(root);
    }
    walk.hidden(false).filter_entry(|e| e.file_name() != ".git");
    if let Some(g) = glob {
        let base = if roots.len() == 1 { &roots[0] } else { cwd };
        let mut o = OverrideBuilder::new(base);
        o.add(&g).map_err(|e| fail(format!("bad glob: {e}")))?;
        walk.overrides(o.build().map_err(|e| fail(e.to_string()))?);
    }
    let (mut out, mut found, mut cut) = (Vec::new(), 0usize, false);
    'files: for entry in walk.build().flatten() {
        if stopped() {
            return Err(fail("grep was stopped"));
        }
        if !entry.file_type().is_some_and(|t| t.is_file()) {
            continue;
        }
        let Ok(file) = std::fs::File::open(entry.path()) else {
            continue;
        };
        let mut reader = BufReader::new(file);
        if reader
            .fill_buf()
            .is_ok_and(|b| b.iter().take(8192).any(|&c| c == 0))
        {
            continue;
        }
        let lines: Vec<String> = reader
            .split(b'\n')
            .map_while(Result::ok)
            .map(|l| {
                String::from_utf8_lossy(&l)
                    .trim_end_matches('\r')
                    .to_string()
            })
            .collect();
        let name = entry.path().strip_prefix(cwd).unwrap_or(entry.path());
        let mut last_shown = 0usize;
        for (i, line) in lines.iter().enumerate() {
            if !re.is_match(line) {
                continue;
            }
            if found == limit {
                cut = true;
                break 'files;
            }
            found += 1;
            let (from, to) = (
                i.saturating_sub(context),
                (i + context + 1).min(lines.len()),
            );
            if last_shown > 0 && from + 1 > last_shown + 1 && context > 0 {
                out.push("--".to_string());
            }
            for (j, l) in lines.iter().enumerate().take(to).skip(from.max(last_shown)) {
                let sep = if j == i { ':' } else { '-' };
                out.push(format!("{}:{}{sep} {}", name.display(), j + 1, clip(l)));
            }
            last_shown = to;
        }
    }
    if out.is_empty() {
        return Ok("[no matches]".into());
    }
    if cut {
        out.push(format!(
            "[stopped at {limit} matches; narrow the pattern or raise limit]"
        ));
    }
    Ok(out.join("\n"))
}

fn clip(l: &str) -> String {
    match l.char_indices().nth(LINE_CHARS) {
        Some((i, _)) => format!("{}…", &l[..i]),
        None => l.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn searches_multiple_paths_and_literal_patterns() {
        let dir = std::env::temp_dir().join(format!("tuse-grep-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("a")).unwrap();
        std::fs::create_dir_all(dir.join("b")).unwrap();
        std::fs::create_dir_all(dir.join("c")).unwrap();
        std::fs::write(dir.join("a/one.txt"), "val[0]+(x)\n").unwrap();
        std::fs::write(dir.join("b/two.txt"), "VAL[0]+(x)\n").unwrap();
        std::fs::write(dir.join("c/skip.txt"), "val[0]+(x)\n").unwrap();
        let out = run(
            "val[0]+(x)",
            Opts {
                paths: vec!["a".into(), "b".into()],
                glob: None,
                ignore_case: true,
                literal: true,
                context: 0,
                limit: None,
            },
            &dir,
            &|| false,
        )
        .unwrap();
        assert!(out.contains("a/one.txt:1: val[0]+(x)"), "{out}");
        assert!(out.contains("b/two.txt:1: VAL[0]+(x)"), "{out}");
        assert!(!out.contains("c/skip.txt"), "{out}");
        std::fs::remove_dir_all(dir).ok();
    }
}
