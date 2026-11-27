use crate::kit::entry::Entry;
use crate::kit::style::{
    ERROR_PREFIX, INPUT_PREFIX, LABEL_SEP, NOTICE_PREFIX, Palette, REASONING_PREFIX, Style,
    UI_MARKDOWN_PREFIX, UI_RESULT_PREFIX,
};
use crate::kit::text::{Line, MessageRow, clip, clip_line, line_width, span, wrap, wrap_message};
use std::time::Duration;

pub fn duration(d: Duration) -> String {
    let s = d.as_secs();
    match s {
        s if s >= 3600 => format!("{}h{}m", s / 3600, s % 3600 / 60),
        s if s >= 60 => format!("{}m{}s", s / 60, s % 60),
        s => format!("{s}s"),
    }
}

pub fn plain(text: &str, width: usize, style: Style, first: &str, rest: &str) -> Vec<Line> {
    let text = &crate::kit::text::clean(text);
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

pub fn chat_inset(width: usize) -> usize {
    2.min(width.saturating_sub(2) / 2)
}

pub fn message_width(width: usize) -> usize {
    width - 2 * chat_inset(width)
}

pub fn pad_message(row: &mut MessageRow, width: usize, st: Style) {
    let inset = chat_inset(width);
    let room = message_width(width);
    let line = &mut row.line;
    if line_width(line) > room {
        *line = clip_line(line, room, "");
    }
    if let Some(copy) = &mut row.copy {
        copy.place(inset, line_width(line));
    }
    if inset > 0 {
        line.insert(0, span(" ".repeat(inset), st));
    }
    let right = width.saturating_sub(line_width(line));
    if right > 0 {
        line.push(span(" ".repeat(right), st));
    }
}

pub fn message(e: &Entry, width: usize, p: &Palette) -> Vec<MessageRow> {
    let room = message_width(width);
    let (mut rows, st) = match e {
        Entry::User { text, images } => {
            let st = Style::fg(p.user).on(p.user_bg);
            let mut text = crate::kit::text::clean(text);
            if *images > 0 && !text.contains("[Image") {
                text.push_str(&format!("\n[{images} image(s)]"));
            }
            let empty = vec![span("", st)];
            let rows = text
                .split('\n')
                .flat_map(|line| wrap_message(&vec![span(line, st)], room, &empty, &empty, 0))
                .collect();
            (rows, st)
        }
        Entry::Assistant(text) => (
            crate::kit::rich::markdown::rows(text, room, p, true),
            Style::default(),
        ),
        Entry::Markdown { text, .. } => return ui_markdown(text, width, p),
        Entry::UiResult {
            is_picker,
            prompt,
            value,
        } => {
            return ui_result(*is_picker, prompt, value, width, p);
        }
        _ => unreachable!(),
    };
    for row in &mut rows {
        pad_message(row, width, st);
    }
    rows
}

pub fn chat_marker(marker: &str, width: usize) -> String {
    clip(marker, chat_inset(width), "")
}

pub fn render(e: &Entry, width: usize, p: &Palette) -> Vec<Line> {
    let pad = " ".repeat(chat_inset(width));
    let marker = |prefix| chat_marker(prefix, width);
    let lines = match e {
        Entry::User { .. }
        | Entry::Assistant(_)
        | Entry::Markdown { .. }
        | Entry::UiResult { .. } => {
            return message(e, width, p)
                .into_iter()
                .map(|row| row.line)
                .collect();
        }
        Entry::Reasoning(text) if text.trim().is_empty() => vec![],
        Entry::Reasoning(text) => {
            let st = Style::fg(p.muted).italic();
            let text = text.trim();
            let cut = usize::try_from(p.ui.lines.reasoning)
                .ok()
                .and_then(|n| text.rmatch_indices('\n').nth(n.saturating_sub(1)));
            let prefix = marker(REASONING_PREFIX);
            let (text, first) = match cut {
                Some((i, _)) => (&text[i + 1..], pad.as_str()),
                None => (text, prefix.as_str()),
            };
            let all = plain(text, width, st, first, &pad);
            let n = tuse_config::limit(p.ui.lines.reasoning, all.len());
            all[all.len() - n..].to_vec()
        }
        Entry::Tool {
            input,
            output,
            code,
            diffs,
            ..
        } => crate::kit::rich::tool_view::tool(input, output.as_deref(), *code, diffs, width, p),
        Entry::Notice(text) => plain(
            text,
            width,
            Style::fg(p.muted),
            &marker(NOTICE_PREFIX),
            &pad,
        ),
        Entry::Error(text) => plain(text, width, Style::fg(p.error), &marker(ERROR_PREFIX), &pad),
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
            plain(
                &text,
                width,
                Style::fg(p.muted),
                &marker(NOTICE_PREFIX),
                &pad,
            )
        }
    };
    fit_chat(lines, width)
}

pub fn fit_chat(mut lines: Vec<Line>, width: usize) -> Vec<Line> {
    for line in &mut lines {
        if line_width(line) > width {
            *line = clip_line(line, width, "");
        }
    }
    lines
}

pub fn ui_markdown(text: &str, width: usize, p: &Palette) -> Vec<MessageRow> {
    marked(UI_MARKDOWN_PREFIX, width, p, |room| {
        crate::kit::rich::markdown::rows(text, room, p, true)
    })
}

fn ui_result(
    is_picker: bool,
    prompt: &str,
    value: &str,
    width: usize,
    p: &Palette,
) -> Vec<MessageRow> {
    let plain_rows = |text: &str, room, st| {
        crate::kit::text::clean(text)
            .split('\n')
            .flat_map(|line| wrap_message(&vec![span(line, st)], room, &vec![], &vec![], 0))
            .collect()
    };
    let mut rows = marked(UI_RESULT_PREFIX, width, p, |room| {
        let mut rows = if is_picker && prompt == "Picker" {
            plain_rows(prompt, room, Style::fg(p.muted))
        } else {
            crate::kit::rich::markdown::rows(prompt, room, p, false)
        };
        if rows.is_empty() {
            rows.push(MessageRow::decoration(vec![]));
        }
        rows
    });
    rows.extend(marked(INPUT_PREFIX, width, p, |room| {
        plain_rows(value, room, Style::fg(p.fg))
    }));
    rows
}

fn marked(
    prefix: &str,
    width: usize,
    p: &Palette,
    render: impl FnOnce(usize) -> Vec<MessageRow>,
) -> Vec<MessageRow> {
    let marker = chat_marker(prefix, width);
    let indent = crate::kit::text::width(&marker);
    let room = width - indent;
    let mut rows = render(room);
    for (i, row) in rows.iter_mut().enumerate() {
        if line_width(&row.line) > room {
            row.line = clip_line(&row.line, room, "");
        }
        if let Some(copy) = &mut row.copy {
            copy.place(indent, line_width(&row.line));
        }
        if indent > 0 {
            if i == 0 {
                row.line.insert(0, span(&marker, Style::fg(p.icons)));
            } else if !row.line.is_empty() {
                row.line
                    .insert(0, span(" ".repeat(indent), Style::default()));
            }
        }
    }
    rows
}

pub fn framed(rows: Vec<Line>, w: usize, p: &Palette) -> Vec<Line> {
    let base = Style::fg(p.fg).on(p.bg);
    let mut out: Vec<Line> = vec![];
    for l in rows {
        let line: Line = l
            .into_iter()
            .map(|mut s| {
                s.style.bg = s.style.bg.or(p.bg);
                s
            })
            .collect();
        out.push(line);
    }
    for l in &mut out {
        let fill = w.saturating_sub(line_width(l));
        l.push(span(" ".repeat(fill), base));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ui_entries_use_accent_markers_and_keep_wrapped_content_aligned() {
        let p = Palette::new(&Default::default());
        for (is_picker, prompt) in [
            (true, "Picker"),
            (true, "**Question**"),
            (false, "**Question**"),
        ] {
            let entry = Entry::UiResult {
                is_picker,
                prompt: prompt.into(),
                value: "**answer**".into(),
            };
            let lines = render(&entry, 20, &p);
            assert_eq!(lines[0][0], span("? ", Style::fg(p.accent)));
            assert_eq!(lines[1][0], span("› ", Style::fg(p.accent)));
            assert_eq!(lines[0].iter().any(|s| s.style.bold), prompt != "Picker");
            assert!(
                lines[1]
                    .iter()
                    .any(|s| s.text == "**answer**" && !s.style.bold)
            );
        }
        let lines: Vec<_> = ui_markdown("**one two three**", 9, &p)
            .into_iter()
            .map(|row| row.line)
            .collect();
        assert_eq!(lines[0][0], span("◆ ", Style::fg(p.accent)));
        assert!(lines[0].iter().any(|s| s.text == "one two" && s.style.bold));
        assert_eq!(lines[1][0].text, "  ");
        assert!(lines.iter().all(|line| line_width(line) <= 9));
    }

    #[test]
    fn the_turn_summary_is_muted() {
        let p = Palette::new(&Default::default());
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
        let p = Palette::new(&Default::default());
        let e = Entry::User {
            text: "hi there\nsecond".into(),
            images: 0,
        };
        let lines = render(&e, 20, &p);
        assert_eq!(lines.len(), 2);
        for (l, want) in lines.iter().zip(["hi there", "second"]) {
            let text: String = l.iter().map(|s| s.text.as_str()).collect();
            assert_eq!(text.trim_end(), format!("  {want}"));
            assert_eq!(line_width(l), 20);
            assert!(l.iter().all(|s| s.style.bg == p.user_bg));
        }
    }

    #[test]
    fn user_and_lua_colors_follow_the_theme() {
        let theme = tuse_config::Theme {
            user: "#112233".into(),
            code: tuse_config::CodeTheme {
                keyword: "accent".into(),
                field: "#445566".into(),
                ..Default::default()
            },
            ..Default::default()
        };
        let p = Palette::new(&tuse_config::Ui {
            theme,
            ..Default::default()
        });
        let e = Entry::User {
            text: "hi".into(),
            images: 0,
        };
        assert_eq!(render(&e, 20, &p)[0][0].style.fg, p.user);
        assert_ne!(p.user, p.fg);
        let base = Style::fg(p.fg);
        let kw = crate::kit::rich::syntax::style(crate::kit::rich::syntax::Kind::Keyword, base, &p);
        assert_eq!(kw.fg, p.accent);
        let plain =
            crate::kit::rich::syntax::style(crate::kit::rich::syntax::Kind::Variable, base, &p);
        assert_eq!(plain.fg, p.fg);
        let linked = Palette::new(&Default::default());
        assert_eq!(linked.user, linked.fg);
    }
}
