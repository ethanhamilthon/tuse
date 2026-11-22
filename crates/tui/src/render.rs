use crate::chat::Entry;
use crate::style::{
    ERROR_PREFIX, LABEL_SEP, NOTICE_PREFIX, Palette, REASONING_PREFIX, Style, USER_PREFIX,
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

/// Elapsed time of a tool call: milliseconds under one second, else `duration`.
pub fn call_time(d: Duration) -> String {
    if d.as_secs() == 0 {
        format!("{}ms", d.as_millis())
    } else {
        duration(d)
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
    let pad = " ".repeat(crate::text::width(USER_PREFIX));
    match e {
        Entry::User { text, images } => {
            let st = Style::fg(p.fg).on(p.surface);
            let mut text = text.clone();
            if *images > 0 && !text.contains("[Image") {
                text.push_str(&format!("\n[{images} image(s)]"));
            }
            let mut lines = plain(&text, width, st, USER_PREFIX, &pad);
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
            timer,
            title,
            input,
            output,
            code,
            diffs,
            ..
        } => {
            let mut lines =
                crate::tool_view::tool(title, input, output.as_deref(), *code, diffs, width, p);
            if let Some((start, limit, elapsed)) = timer
                && !title.trim().is_empty()
                && p.ui.tool_line != 0
                && let Some(line) = lines.first_mut()
            {
                let label = format!(
                    " {}/{}",
                    call_time(elapsed.unwrap_or_else(|| start.elapsed())),
                    duration(*limit)
                );
                let left = width.saturating_sub(crate::text::width(&label));
                line.truncate(1);
                let title = clean(&title.split_whitespace().collect::<Vec<_>>().join(" "));
                line.push(span(
                    crate::text::clip(&title, left.saturating_sub(line_width(line)), "…"),
                    Style::fg(p.dim),
                ));
                line.push(span(
                    crate::text::clip(&label, width.saturating_sub(line_width(line)), ""),
                    Style::fg(p.accent),
                ));
            }
            lines
        }
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
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn call_time_uses_milliseconds_under_a_second() {
        let ms = Duration::from_millis;
        assert_eq!(call_time(ms(0)), "0ms");
        assert_eq!(call_time(ms(302)), "302ms");
        assert_eq!(call_time(ms(999)), "999ms");
        assert_eq!(call_time(ms(1000)), "1s");
        assert_eq!(call_time(ms(61_000)), "1m1s");
    }
}
