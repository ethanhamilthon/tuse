use super::resolve;
use ignore::WalkBuilder;
use ignore::overrides::OverrideBuilder;
use regex::RegexBuilder;
use std::io::{BufRead, BufReader};
use std::path::Path;
use tuse_py::{Args, Ctx, ExcType, Fail, MontyObject};

const DEFAULT_LIMIT: usize = 100;
/// Characters of a matching line shown.
const LINE_CHARS: usize = 300;

/// `grep(pattern, path=".", glob=None, ignore_case=False, context=0, limit=100)`
pub fn call(a: &Args, ctx: &Ctx, cwd: &Path) -> Result<MontyObject, Fail> {
    let pattern = a.str(0, "pattern")?;
    let root = resolve(&a.opt_str(1, "path")?.unwrap_or_else(|| ".".into()), cwd);
    let glob = a.opt_str(2, "glob")?;
    let ignore_case = a.bool(3, "ignore_case", false)?;
    let context = a.opt_int(4, "context")?.map_or(0, |n| n.max(0) as usize);
    let limit = a
        .opt_int(5, "limit")?
        .map_or(DEFAULT_LIMIT, |n| n.max(1) as usize);
    let re = RegexBuilder::new(&pattern)
        .case_insensitive(ignore_case)
        .build()
        .map_err(|e| Fail::value(format!("bad pattern: {e}")))?;
    if !root.exists() {
        return Err(Fail::of(
            ExcType::FileNotFoundError,
            format!("{}: no such file or folder", root.display()),
        ));
    }
    let mut walk = WalkBuilder::new(&root);
    walk.hidden(false).filter_entry(|e| e.file_name() != ".git");
    if let Some(g) = glob {
        let mut o = OverrideBuilder::new(&root);
        o.add(&g)
            .map_err(|e| Fail::value(format!("bad glob: {e}")))?;
        walk.overrides(o.build().map_err(|e| Fail::value(e.to_string()))?);
    }
    let (mut out, mut found, mut cut) = (Vec::new(), 0usize, false);
    'files: for entry in walk.build().flatten() {
        if ctx.stopped() {
            return Err(Fail::of(ExcType::TimeoutError, "grep was stopped"));
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
        return Ok(MontyObject::string("[no matches]"));
    }
    if cut {
        out.push(format!(
            "[stopped at {limit} matches; narrow the pattern or raise limit]"
        ));
    }
    Ok(MontyObject::string(out.join("\n")))
}

fn clip(l: &str) -> String {
    match l.char_indices().nth(LINE_CHARS) {
        Some((i, _)) => format!("{}…", &l[..i]),
        None => l.to_string(),
    }
}
