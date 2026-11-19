/// One replaced region, as whole lines.
pub struct Hunk {
    /// 1-based line where the region starts in the old text.
    pub start: usize,
    pub old: Vec<String>,
    pub new: Vec<String>,
    pub before: Vec<String>,
    pub after: Vec<String>,
}

const CONTEXT: usize = 2;
const MAX_HUNKS: usize = 5;

/// Hunks for replacing the byte ranges `at` (start, old length) of `text` with `new`.
pub fn hunks(text: &str, at: &[(usize, usize)], new: &str) -> Vec<Hunk> {
    let lines: Vec<&str> = text.split('\n').collect();
    let starts: Vec<usize> = std::iter::once(0)
        .chain(text.match_indices('\n').map(|(i, _)| i + 1))
        .collect();
    let line_of = |b: usize| starts.partition_point(|&s| s <= b) - 1;
    at.iter()
        .map(|&(s, len)| {
            let (first, last) = (line_of(s), line_of((s + len).saturating_sub(1).max(s)));
            let head = &text[starts[first]..s];
            let tail_from = s + len;
            let tail_to = starts.get(last + 1).map_or(text.len(), |e| e - 1);
            let tail = &text[tail_from.min(tail_to)..tail_to];
            let clean = |s: &str| {
                s.split('\n')
                    .map(|l| l.trim_end_matches('\r').to_string())
                    .collect()
            };
            let ctx = |r: std::ops::Range<usize>| -> Vec<String> {
                lines[r]
                    .iter()
                    .map(|l| l.trim_end_matches('\r').to_string())
                    .collect()
            };
            Hunk {
                start: first + 1,
                old: ctx(first..last + 1),
                new: clean(&format!("{head}{new}{tail}")),
                before: ctx(first.saturating_sub(CONTEXT)..first),
                after: ctx((last + 1).min(lines.len())..(last + 1 + CONTEXT).min(lines.len())),
            }
        })
        .collect()
}

/// `  12 | context`, `- 13 | removed`, `+ 13 | added`; line numbers of added lines
/// account for the earlier hunks.
pub fn render(hunks: &[Hunk]) -> String {
    let mut out = String::new();
    let mut shift = 0isize;
    for h in hunks.iter().take(MAX_HUNKS) {
        let first_ctx = h.start - h.before.len();
        for (i, l) in h.before.iter().enumerate() {
            out.push_str(&format!("  {:>4} | {l}\n", first_ctx + i));
        }
        for (i, l) in h.old.iter().enumerate() {
            out.push_str(&format!("- {:>4} | {l}\n", h.start + i));
        }
        let new_start = (h.start as isize + shift) as usize;
        for (i, l) in h.new.iter().enumerate() {
            out.push_str(&format!("+ {:>4} | {l}\n", new_start + i));
        }
        let after_start = h.start + h.old.len();
        for (i, l) in h.after.iter().enumerate() {
            out.push_str(&format!("  {:>4} | {l}\n", after_start + i));
        }
        shift += h.new.len() as isize - h.old.len() as isize;
        out.push('\n');
    }
    if hunks.len() > MAX_HUNKS {
        out.push_str(&format!(
            "[{} more replacements]\n",
            hunks.len() - MAX_HUNKS
        ));
    }
    out.trim_end().to_string()
}
