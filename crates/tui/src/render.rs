use crate::chat::Entry;
use crate::style::{
    ERROR_PREFIX, INPUT_PREFIX, LABEL_SEP, NOTICE_PREFIX, Palette, REASONING_PREFIX, Style,
};
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
    let pad = " ".repeat(crate::text::width(NOTICE_PREFIX));
    match e {
        Entry::User { text, images } => {
            let st = Style::fg(p.fg).on(p.surface);
            let mut text = text.clone();
            if *images > 0 && !text.contains("[Image") {
                text.push_str(&format!("\n[{images} image(s)]"));
            }
            // No prefix: the `surface` block marks the message.
            let mut lines = plain(&text, width, st, "", "");
            for l in &mut lines {
                let fill = width.saturating_sub(line_width(l));
                l.push(span(" ".repeat(fill), st));
            }
            lines
        }
        Entry::Assistant(text) => crate::markdown::render(text, width, p),
        Entry::Reasoning(text) if text.trim().is_empty() => vec![],
        Entry::Reasoning(text) => {
            let st = Style::fg(p.muted).italic();
            let text = text.trim();
            // Only the last lines are shown; wrapping the rest is wasted work.
            let cut = usize::try_from(p.ui.reasoning_line)
                .ok()
                .and_then(|n| text.rmatch_indices('\n').nth(n.saturating_sub(1)));
            let (text, first) = match cut {
                Some((i, _)) => (&text[i + 1..], pad.as_str()),
                None => (text, REASONING_PREFIX),
            };
            let all = plain(text, width, st, first, &pad);
            let n = tuse_config::limit(p.ui.reasoning_line, all.len());
            all[all.len() - n..].to_vec()
        }
        Entry::Tool {
            input,
            output,
            code,
            diffs,
            ..
        } => crate::tool_view::tool(input, output.as_deref(), *code, diffs, width, p),
        Entry::Notice(text) => plain(text, width, Style::fg(p.dim), NOTICE_PREFIX, &pad),
        Entry::Error(text) => plain(text, width, Style::fg(p.err), ERROR_PREFIX, &pad),
        Entry::TurnEnd {
            elapsed,
            model,
            effort,
            cancelled,
        } => {
            let mut parts = vec![];
            if *cancelled {
                parts.push("cancelled".to_string());
            }
            parts.push(duration(*elapsed));
            if !model.is_empty() {
                parts.push(format!("{model}/{effort}"));
            }
            let text = parts.join(LABEL_SEP);
            plain(&text, width, Style::fg(p.dim), NOTICE_PREFIX, &pad)
        }
        Entry::Markdown { text, .. } => crate::markdown::render(text, width, p),
        Entry::UiResult {
            is_picker,
            prompt,
            value,
        } => {
            let mut out = if *is_picker {
                plain(prompt, width, Style::fg(p.dim), "", "")
            } else {
                crate::markdown::render(prompt, width, p)
            };
            let indent = " ".repeat(crate::text::width(INPUT_PREFIX));
            let first = vec![span(INPUT_PREFIX, Style::fg(p.accent))];
            let rest = vec![span(indent, Style::fg(p.accent))];
            let st = Style::fg(p.fg);
            for (i, l) in clean(value).split('\n').enumerate() {
                out.extend(wrap(
                    &vec![span(l, st)],
                    width,
                    if i == 0 { &first } else { &rest },
                    &rest,
                ));
            }
            out
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_user_message_is_a_surface_block_without_a_prefix() {
        let p = Palette::new(&Default::default(), &Default::default());
        let e = Entry::User {
            text: "hi there\nsecond".into(),
            images: 0,
        };
        let lines = render(&e, 20, &p);
        assert_eq!(lines.len(), 2);
        for (l, want) in lines.iter().zip(["hi there", "second"]) {
            let text: String = l.iter().map(|s| s.text.as_str()).collect();
            assert_eq!(text.trim_end(), want);
            assert_eq!(line_width(l), 20);
            assert!(l.iter().all(|s| s.style.bg == p.surface));
        }
    }
}
