use crate::style::{Palette, Style};
use crate::text::{Line, line_width, span, wrap};
use std::time::Duration;
use tuse_core::Record;

#[derive(Debug, Clone)]
pub enum Entry {
    User {
        text: String,
        images: usize,
    },
    Assistant(String),
    Reasoning(String),
    Tool {
        id: String,
        command: String,
        output: Option<String>,
        code: Option<i32>,
    },
    Notice(String),
    Error(String),
    TurnEnd {
        elapsed: Duration,
        calls: usize,
        cancelled: bool,
    },
}

/// Wrapped lines of an entry and the width they were wrapped to.
type Cache = Option<(usize, Vec<Line>)>;

/// Chat entries with their wrapped lines cached per width.
#[derive(Default)]
pub struct Chat {
    entries: Vec<(Entry, Cache)>,
}

impl Chat {
    pub fn from_records(records: &[Record]) -> Chat {
        let mut c = Chat::default();
        for r in records {
            let e = match r {
                Record::User { text, images, .. } => Entry::User {
                    text: text.clone(),
                    images: images.len(),
                },
                Record::Assistant {
                    text, reasoning, ..
                } => {
                    if !reasoning.trim().is_empty() {
                        c.push(Entry::Reasoning(reasoning.clone()));
                    }
                    if text.trim().is_empty() {
                        continue;
                    }
                    Entry::Assistant(text.clone())
                }
                Record::Tool {
                    id,
                    command,
                    output,
                    code,
                    ..
                } => Entry::Tool {
                    id: id.clone(),
                    command: command.clone(),
                    output: Some(output.clone()),
                    code: *code,
                },
                Record::Notice { text } => Entry::Notice(text.clone()),
                Record::Error { text } => Entry::Error(text.clone()),
                Record::TurnEnd {
                    elapsed_ms,
                    calls,
                    cancelled,
                } => Entry::TurnEnd {
                    elapsed: Duration::from_millis(*elapsed_ms),
                    calls: *calls,
                    cancelled: *cancelled,
                },
                _ => continue,
            };
            c.push(e);
        }
        c
    }

    pub fn push(&mut self, e: Entry) {
        self.entries.push((e, None));
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn get_mut(&mut self, i: usize) -> Option<&mut Entry> {
        let slot = self.entries.get_mut(i)?;
        slot.1 = None;
        Some(&mut slot.0)
    }

    pub fn remove(&mut self, i: usize) {
        if i < self.entries.len() {
            self.entries.remove(i);
        }
    }

    pub fn find_tool(&mut self, id: &str) -> Option<&mut Entry> {
        let i = self
            .entries
            .iter()
            .rposition(|(e, _)| matches!(e, Entry::Tool { id: x, .. } if x == id))?;
        self.get_mut(i)
    }

    pub fn lines(&mut self, width: usize, p: &Palette) -> Vec<Line> {
        let mut out = vec![];
        for (i, (e, cache)) in self.entries.iter_mut().enumerate() {
            if cache.as_ref().is_none_or(|(w, _)| *w != width) {
                *cache = Some((width, render(e, width, p)));
            }
            let lines = &cache.as_ref().unwrap().1;
            if lines.is_empty() {
                continue;
            }
            if i > 0 && !out.is_empty() {
                out.extend((0..p.t.message_gap).map(|_| vec![]));
            }
            out.extend(lines.iter().cloned());
        }
        out
    }
}

pub fn duration(d: Duration) -> String {
    let s = d.as_secs();
    match s {
        s if s >= 3600 => format!("{}h{}m", s / 3600, s % 3600 / 60),
        s if s >= 60 => format!("{}m{}s", s / 60, s % 60),
        s => format!("{s}s"),
    }
}

fn plain(text: &str, width: usize, style: Style, first: &str, rest: &str) -> Vec<Line> {
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

fn render(e: &Entry, width: usize, p: &Palette) -> Vec<Line> {
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
            command,
            output,
            code,
            ..
        } => tool(command, output.as_deref(), *code, width, p),
        Entry::Notice(text) => plain(text, width, Style::fg(p.notice), &t.notice_prefix, &pad),
        Entry::Error(text) => plain(text, width, Style::fg(p.error), &t.error_prefix, &pad),
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

fn tool(
    command: &str,
    output: Option<&str>,
    code: Option<i32>,
    width: usize,
    p: &Palette,
) -> Vec<Line> {
    let t = &p.t;
    let failed = output.is_some() && code != Some(0);
    let head = Style::fg(if failed { p.tool_error } else { p.tool });
    let pad = " ".repeat(crate::text::width(&t.tool_prefix));
    let mut lines = plain(command.trim(), width, head, &t.tool_prefix, &pad);
    let keep = tuse_config::limit(t.bash_line, lines.len());
    if keep < lines.len() {
        lines.truncate(keep);
        if keep > 0 {
            lines.push(vec![span(format!("{pad}{}", t.more), Style::fg(p.dim))]);
        }
    }
    let Some(out) = output else { return lines };
    let st = Style::fg(p.tool_output);
    let all: Vec<&str> = out.trim_end().lines().collect();
    let shown = tuse_config::limit(t.output_line, all.len());
    for l in all.iter().take(shown) {
        let l = crate::text::clip(l, width.saturating_sub(pad.len()), &t.more);
        lines.push(vec![span(pad.clone(), st), span(l, st)]);
    }
    if shown > 0 && shown < all.len() {
        let more = format!("{pad}{} {} more lines", t.more, all.len() - shown);
        lines.push(vec![span(more, Style::fg(p.dim))]);
    }
    lines
}
