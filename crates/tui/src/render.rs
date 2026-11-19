use crate::chat::Entry;
use crate::style::{Palette, Style};
use crate::text::{Line, line_width, span, wrap};
use std::time::Duration;

pub fn duration(d: Duration) -> String {
    let s = d.as_secs();
    match s {
        s if s >= 3600 => format!("{}h{}m", s / 3600, s % 3600 / 60),
        s if s >= 60 => format!("{}m{}s", s / 60, s % 60),
        s => format!("{s}s"),
    }
}

/// Tabs become spaces and control characters (ESC included) are dropped,
/// so text from scripts cannot move the cursor or break the width.
pub(crate) fn clean(text: &str) -> String {
    text.chars()
        .filter(|c| *c == '\n' || *c == '\t' || !c.is_control())
        .map(|c| {
            if c == '\t' {
                "    ".to_string()
            } else {
                c.to_string()
            }
        })
        .collect()
}

pub fn plain(text: &str, width: usize, style: Style, first: &str, rest: &str) -> Vec<Line> {
    let text = &clean(text);
    let (f, r) = (vec![span(first, style)], vec![span(rest, style)]);
    text.split('\n')
        .enumerate()
        .flat_map(|(i, l)| {
            wrap(
                &vec![span(l, style)],
                width,
                if i == 0 { &f } else { &r },
                &r,
            )
        })
        .collect()
}

pub fn render(e: &Entry, width: usize, p: &Palette) -> Vec<Line> {
    let t = &p.t;
    let pad = " ".repeat(crate::text::width(&t.user_prefix));
    match e {
        Entry::User { text, images } => {
            let st = Style::fg(p.user).on(p.user_bg);
            let mut text = text.clone();
            if *images > 0 && !text.contains("[Image") {
                text.push_str(&format!("\n[{images} image(s)]"));
            }
            let mut lines = plain(&text, width, st, &t.user_prefix, &pad);
            for l in &mut lines {
                let fill = width.saturating_sub(line_width(l));
                l.push(span(" ".repeat(fill), st));
            }
            lines
        }
        Entry::Assistant(text) => crate::markdown::render(text, width, p),
        Entry::Reasoning(text) if text.trim().is_empty() => vec![],
        Entry::Reasoning(text) => {
            let st = Style::fg(p.reasoning).italic();
            let all = plain(text.trim(), width, st, &t.reasoning_prefix, &pad);
            let n = tuse_config::limit(t.reasoning_line, all.len());
            all[all.len() - n..].to_vec()
        }
        Entry::Tool {
            input,
            output,
            code,
            diffs,
            ..
        } => crate::tool_view::tool(input, output.as_deref(), *code, diffs, width, p),
        Entry::Notice(text) => plain(text, width, Style::fg(p.notice), &t.notice_prefix, &pad),
        Entry::Error(text) => plain(text, width, Style::fg(p.error), &t.error_prefix, &pad),
        Entry::Print { text, .. } => plain(text, width, Style::fg(p.fg), "", ""),
        Entry::TurnEnd {
            elapsed,
            calls,
            cancelled,
        } => {
            let word = if *cancelled {
                "Cancelled after"
            } else {
                "Done in"
            };
            let text = format!("{word} {}{}{calls} calls", duration(*elapsed), t.label_sep);
            plain(&text, width, Style::fg(p.notice), &t.notice_prefix, &pad)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_drops_escapes_and_expands_tabs() {
        let got = plain("a\tb\n\x1b[31mred", 40, Style::default(), "", "");
        let text: Vec<String> = got
            .iter()
            .map(|l| l.iter().map(|s| s.text.as_str()).collect())
            .collect();
        assert_eq!(text, ["a    b", "[31mred"]);
    }
}
