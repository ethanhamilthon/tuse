use crate::style::{MORE, Palette, Style, TOOL_PREFIX};
use crate::text::{Line, clip, span, width};
use tuse_core::FileDiff;

/// A `lua` call in the chat: its code (colored), then its output, then the
/// diff of every file it changed.
pub fn tool(
    title: &str,
    input: &str,
    output: Option<&str>,
    code: Option<i32>,
    diffs: &[FileDiff],
    width: usize,
    p: &Palette,
) -> Vec<Line> {
    let failed = output.is_some() && code != Some(0);
    let head = Style::fg(if failed { p.err } else { p.info });
    let pad = " ".repeat(crate::text::width(TOOL_PREFIX));
    let title = title.split_whitespace().collect::<Vec<_>>().join(" ");
    let title = crate::render::clean(&title);
    let titled = !title.trim().is_empty();
    let mut lines = source(input, width, head, failed, titled, p);
    let keep = tuse_config::limit(p.ui.tool_line, lines.len());
    if keep < lines.len() {
        lines.truncate(keep);
        if keep > 0 {
            lines.push(vec![span(format!("{pad}{MORE}"), Style::fg(p.dim))]);
        }
    }
    if titled && p.ui.tool_line != 0 {
        lines.insert(
            0,
            vec![
                span(clip(TOOL_PREFIX, width, ""), Style::fg(p.dim)),
                span(
                    clip(
                        &title,
                        width.saturating_sub(pad.len()),
                        if width > pad.len() { MORE } else { "" },
                    ),
                    Style::fg(p.dim),
                ),
            ],
        );
    }
    let Some(out) = output else { return lines };
    let out = crate::render::clean(out);
    let all: Vec<&str> = out.trim_end().lines().collect();
    let shown = tuse_config::limit(p.ui.output_line, all.len());
    let st = Style::fg(p.border);
    for l in all.iter().take(shown) {
        let l = clip(l, width.saturating_sub(pad.len()), MORE);
        lines.push(vec![span(pad.clone(), st), span(l, st)]);
    }
    if shown > 0 && shown < all.len() {
        let more = format!("{pad}{MORE} {} more lines", all.len() - shown);
        lines.push(vec![span(more, Style::fg(p.dim))]);
    }
    for d in diffs {
        lines.extend(diff(d, width, &pad, p));
    }
    lines
}

/// The code wrapped to `width`, colored unless the call failed.
fn source(
    input: &str,
    w: usize,
    head: Style,
    failed: bool,
    titled: bool,
    p: &Palette,
) -> Vec<Line> {
    let cleaned = crate::render::clean(input.trim());
    let code = if p.ui.fold_strings {
        crate::fold::fold_multiline_strings(&cleaned)
    } else {
        cleaned
    };
    let pad = " ".repeat(width(TOOL_PREFIX));
    let colored = if failed {
        code.split('\n').map(|l| vec![span(l, head)]).collect()
    } else {
        crate::syntax::highlight(&code, head, p)
    };
    let (first, rest) = (vec![span(TOOL_PREFIX, head)], vec![span(pad, head)]);
    colored
        .into_iter()
        .enumerate()
        .flat_map(|(i, l)| {
            crate::text::wrap(&l, w, if i == 0 && !titled { &first } else { &rest }, &rest)
        })
        .collect()
}

/// `path` and its changed lines, colored; a long changed line shows the part around the change.
fn diff(d: &FileDiff, w: usize, pad: &str, p: &Palette) -> Vec<Line> {
    let room = w.saturating_sub(pad.len());
    let all: Vec<&str> = d.diff.lines().collect();
    let mut out = vec![vec![span(
        format!("{pad}{}", clip(&d.path, room, MORE)),
        Style::fg(p.border),
    )]];
    let shown = tuse_config::limit(p.ui.diff_line, all.len());
    let pairs = pair_windows(&all[..shown], room, MORE);
    for (l, text) in all.iter().zip(pairs) {
        let color = match l.chars().next() {
            Some('+') => p.ok,
            Some('-') => p.err,
            _ => p.dim,
        };
        let st = Style::fg(color);
        out.push(vec![span(pad.to_string(), st), span(text, st)]);
    }
    if shown < all.len() {
        let more = format!("{pad}{MORE} {} more lines", all.len() - shown);
        out.push(vec![span(more, Style::fg(p.dim))]);
    }
    out
}

/// Fits each diff line to `room` columns. A `-` line followed by a `+` line is cut
/// around their first difference, so the change stays visible at the end of a long line.
fn pair_windows(lines: &[&str], room: usize, more: &str) -> Vec<String> {
    let mut out: Vec<String> = lines
        .iter()
        .map(|l| clip(&crate::render::clean(l), room, more))
        .collect();
    for i in 0..lines.len().saturating_sub(1) {
        let (a, b) = (
            crate::render::clean(lines[i]),
            crate::render::clean(lines[i + 1]),
        );
        if !(a.starts_with('-') && b.starts_with('+')) || width(&a) <= room && width(&b) <= room {
            continue;
        }
        // The gutter (`- 12 | `) is kept; the text after it is windowed around the first difference.
        let gutter = a.find("| ").map_or(0, |g| a[..g + 2].chars().count());
        let same = a.chars().skip(gutter).zip(b.chars().skip(gutter));
        let at = gutter + same.take_while(|(x, y)| x == y).count();
        let start = if at + 8 > room {
            at.saturating_sub(room / 2).max(gutter)
        } else {
            0
        };
        for (j, s) in [(i, &a), (i + 1, &b)] {
            let head: String = s.chars().take(gutter).collect();
            let body: String = s.chars().skip(start.max(gutter)).collect();
            let lead = if start > gutter { more } else { "" };
            let text = format!("{head}{lead}{body}");
            out[j] = clip(&text, room, more);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_change_at_the_end_of_a_long_line_stays_visible() {
        let a = format!("- {:>4} | {}old", 1, "x".repeat(80));
        let b = format!("+ {:>4} | {}new", 1, "x".repeat(80));
        let got = pair_windows(&[&a, &b], 40, "…");
        assert!(
            got[0].ends_with("old") && got[1].ends_with("new"),
            "{got:?}"
        );
        assert!(got.iter().all(|l| width(l) <= 40));
    }

    #[test]
    fn title_uses_its_own_row_and_color() {
        let mut p = Palette::new(&Default::default(), &Default::default());
        p.ui.tool_line = 1;
        let preview = |title: &str, p: &Palette| {
            tool(title, "first\nsecond", Some("result"), Some(0), &[], 9, p)
        };
        let text = |line: &Line| line.iter().map(|s| s.text.as_str()).collect::<String>();
        let lines = preview(" Test\n long\x1b title ", &p);
        assert_eq!(text(&lines[0]), "* Test l…");
        assert_eq!(lines[0][0].style.fg, p.dim);
        assert_eq!(lines[0][1].style.fg, p.dim);
        assert_eq!(text(&lines[1]), "  first");
        assert_eq!(text(&preview(" ", &p)[0]), "* first");
        p.ui.tool_line = 0;
        let hidden = preview("Test", &p);
        assert_eq!(hidden.len(), 1);
        assert_eq!(text(&hidden[0]), "  result");
    }

    #[test]
    fn tool_output_tabs_do_not_reach_the_terminal() {
        let p = Palette::new(&Default::default(), &Default::default());
        let lines = tool(
            "",
            "x = 1",
            Some("1\tx\n2\t\x1b[31my"),
            Some(0),
            &[],
            40,
            &p,
        );
        let text: String = lines.iter().flatten().map(|s| s.text.as_str()).collect();
        assert!(!text.contains('\t') && !text.contains('\x1b'), "{text:?}");
        assert!(text.contains("1    x"), "tabs become spaces: {text:?}");
    }
}
