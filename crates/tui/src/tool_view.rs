use crate::style::{MORE, Palette, Style, TOOL_PREFIX};
use crate::text::{Line, clip, span, width};
use tuse_core::FileDiff;

pub fn tool(
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
    let mut lines = source(input, width, head, failed, p);
    let keep = tuse_config::limit(p.ui.tool_line, lines.len());
    if keep < lines.len() {
        lines.truncate(keep);
        if keep > 0 {
            lines.push(vec![span(format!("{pad}{MORE}"), Style::fg(p.border))]);
        }
    }
    let Some(out) = output else { return lines };
    let out = crate::render::clean(out);
    let all: Vec<&str> = out.trim_end().lines().collect();
    let shown = tuse_config::limit(p.ui.output_line, all.len());
    let st = Style::fg(p.muted);
    let room = width.saturating_sub(pad.len());
    for l in all.iter().take(shown) {
        lines.push(vec![span(pad.clone(), st), span(clip(l, room, MORE), st)]);
    }
    if shown > 0 && shown < all.len() {
        let more = format!("{pad}{MORE} {} more lines", all.len() - shown);
        lines.push(vec![span(more, Style::fg(p.border))]);
    }
    for d in diffs {
        lines.extend(diff(d, width, &pad, p));
    }
    lines
}

fn source(input: &str, w: usize, head: Style, failed: bool, p: &Palette) -> Vec<Line> {
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
        .flat_map(|(i, l)| crate::text::wrap(&l, w, if i == 0 { &first } else { &rest }, &rest))
        .collect()
}

fn diff(d: &FileDiff, w: usize, pad: &str, p: &Palette) -> Vec<Line> {
    let room = w.saturating_sub(pad.len());
    let all: Vec<&str> = d.diff.lines().collect();
    let hdr = span(
        clip(&crate::render::clean(&d.path), room, MORE),
        Style::fg(p.border),
    );
    let mut out = vec![vec![span(pad.to_string(), Style::fg(p.border)), hdr]];
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
        out.push(vec![span(more, Style::fg(p.border))]);
    }
    out
}

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
    fn output_is_muted_and_cut_marks_are_border() {
        let p = Palette::new(&Default::default(), &Default::default());
        let fg = |l: &Line| l.last().unwrap().style.fg;
        let lines = tool(
            "a = 1\nb = 2",
            Some("one\ntwo\nthree"),
            Some(0),
            &[],
            40,
            &p,
        );
        let n = lines.len();
        assert_eq!(fg(&lines[n - 2]), p.muted, "the shown output line");
        assert!(lines[n - 1].last().unwrap().text.ends_with("2 more lines"));
        assert_eq!(fg(&lines[n - 1]), p.border, "`N more lines`");

        let ui = tuse_config::UiSettings {
            tool_line: 1,
            ..Default::default()
        };
        let p = Palette::new(&Default::default(), &ui);
        let lines = tool("a = 1\nb = 2", None, None, &[], 40, &p);
        assert_eq!(lines.len(), 2);
        assert!(lines[1][0].text.ends_with(MORE));
        assert_eq!(fg(&lines[1]), p.border, "the cut code mark");

        let d = FileDiff {
            path: "f".into(),
            diff: (0..20).map(|i| format!("+ {i}\n")).collect(),
        };
        let lines = tool("x", Some(""), Some(0), &[d], 40, &p);
        let last = lines.last().unwrap();
        assert!(last[0].text.ends_with("8 more lines"), "{last:?}");
        assert_eq!(fg(last), p.border, "`N more lines` under a diff");
    }

    #[test]
    fn tool_output_tabs_do_not_reach_the_terminal() {
        let p = Palette::new(&Default::default(), &Default::default());
        let lines = tool("x = 1", Some("1\tx\n2\t\x1b[31my"), Some(0), &[], 40, &p);
        let text: String = lines.iter().flatten().map(|s| s.text.as_str()).collect();
        assert!(!text.contains('\t') && !text.contains('\x1b'), "{text:?}");
        assert!(text.contains("1    x"), "tabs become spaces: {text:?}");
    }
}
