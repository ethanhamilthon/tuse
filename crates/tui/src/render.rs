use crate::chat::Entry;
use crate::style::{
    ERROR_PREFIX, INPUT_PREFIX, LABEL_SEP, NOTICE_PREFIX, Palette, REASONING_PREFIX, Style,
    UI_MARKDOWN_PREFIX, UI_RESULT_PREFIX,
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
            plain(&text, width, Style::fg(p.muted), NOTICE_PREFIX, &pad)
        }
        Entry::Markdown { text, .. } => ui_markdown(text, width, p),
        Entry::UiResult {
            is_picker,
            prompt,
            value,
        } => marked(UI_RESULT_PREFIX, width, p, |room| {
            let mut out = if *is_picker {
                plain(prompt, room, Style::fg(p.dim), "", "")
            } else {
                crate::markdown::render_with_links(prompt, room, p, false)
            };
            if out.is_empty() {
                out.push(vec![]);
            }
            let answer = marked(INPUT_PREFIX, room, p, |room| {
                plain(value, room, Style::fg(p.fg), "", "")
            });
            out.extend(answer);
            out
        }),
    }
}

pub fn ui_markdown(text: &str, width: usize, p: &Palette) -> Vec<Line> {
    marked(UI_MARKDOWN_PREFIX, width, p, |room| {
        crate::markdown::render(text, room, p)
    })
}

fn marked(
    marker: &str,
    width: usize,
    p: &Palette,
    render: impl FnOnce(usize) -> Vec<Line>,
) -> Vec<Line> {
    let marker = crate::text::clip(marker, width.saturating_sub(1), "");
    let indent = crate::text::width(&marker);
    let mut lines = render(width.saturating_sub(indent));
    if !marker.is_empty() {
        for (i, line) in lines.iter_mut().enumerate() {
            if i == 0 {
                line.insert(0, span(marker.clone(), Style::fg(p.accent)));
            } else if !line.is_empty() {
                line.insert(0, span(" ".repeat(indent), Style::default()));
            }
        }
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ui_entries_use_accent_markers_and_keep_wrapped_content_aligned() {
        let p = Palette::new(&Default::default(), &Default::default());
        for (is_picker, prompt) in [(true, "Picker"), (false, "**Question**")] {
            let entry = Entry::UiResult {
                is_picker,
                prompt: prompt.into(),
                value: "answer".into(),
            };
            let lines = render(&entry, 20, &p);
            assert_eq!(lines[0][0], span("? ", Style::fg(p.accent)));
            assert_eq!(lines[1][0].text, "  ");
            assert_eq!(lines[1][1], span("› ", Style::fg(p.accent)));
        }
        let lines = ui_markdown("**one two three**", 9, &p);
        assert_eq!(lines[0][0], span("◆ ", Style::fg(p.accent)));
        assert!(lines[0].iter().any(|s| s.text == "one two" && s.style.bold));
        assert_eq!(lines[1][0].text, "  ");
        assert!(lines.iter().all(|line| line_width(line) <= 9));
    }

    #[test]
    fn the_turn_summary_is_muted() {
        let p = Palette::new(&Default::default(), &Default::default());
        let entry = Entry::TurnEnd {
            elapsed: Duration::from_secs(70),
            model: "provider/model".into(),
            effort: "medium".into(),
            cancelled: false,
        };
        assert_eq!(
            render(&entry, 80, &p),
            vec![vec![span(
                "✓ 1m10s · provider/model/medium",
                Style::fg(p.muted)
            )]],
        );
    }

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
