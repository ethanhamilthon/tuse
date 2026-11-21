use crate::render::render;
use crate::style::Palette;
use crate::text::Line;
use std::time::Duration;
use tuse_core::{FileDiff, Record};

#[derive(Debug, Clone)]
pub enum Entry {
    User {
        text: String,
        images: usize,
    },
    Assistant(String),
    Reasoning(String),
    Tool {
        timer: Option<(std::time::Instant, Duration, Option<Duration>)>,
        id: String,
        title: String,
        input: String,
        output: Option<String>,
        code: Option<i32>,
        diffs: Vec<FileDiff>,
    },
    Notice(String),
    Error(String),
    TurnEnd {
        elapsed: Duration,
        model: String,
        effort: String,
        cancelled: bool,
    },
    /// A plain text block from `draw_ui()`.
    Print {
        id: String,
        text: String,
    },
}

/// Wrapped lines of an entry and the width they were wrapped to.
type Cache = Option<(usize, Vec<Line>)>;

/// Chat entries with their wrapped lines cached per width.
#[derive(Default)]
pub struct Chat {
    pub(crate) entries: Vec<(Entry, Cache)>,
    /// Pinned `draw_ui` blocks in creation order; always rendered after `entries`.
    pub(crate) pins: Vec<(Entry, Cache)>,
    pub(crate) layout: crate::chat_anchor::Layout,
    pub(crate) pending: Option<crate::chat_anchor::Anchor>,
    /// The reply that is changing (streaming) and its rendered finished blocks.
    hot: Option<usize>,
    stream: Option<crate::md_stream::Prefix>,
}

impl Chat {
    pub fn from_records(records: &[Record], display: &dyn Fn(&str) -> String) -> Chat {
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
                    title,
                    input,
                    output,
                    code,
                    diffs,
                    ..
                } => Entry::Tool {
                    timer: None,
                    id: id.clone(),
                    title: title.clone(),
                    input: input.clone(),
                    output: Some(output.clone()),
                    code: *code,
                    diffs: diffs.clone(),
                },
                Record::Notice { text } => Entry::Notice(text.clone()),
                Record::Error { text } => Entry::Error(text.clone()),
                Record::TurnEnd {
                    elapsed_ms,
                    model,
                    effort,
                    cancelled,
                    ..
                } => Entry::TurnEnd {
                    elapsed: Duration::from_millis(*elapsed_ms),
                    model: if model.is_empty() {
                        String::new()
                    } else {
                        display(model)
                    },
                    effort: effort.clone(),
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
        if matches!(slot.0, Entry::Assistant(_)) {
            self.hot = Some(i);
        }
        Some(&mut slot.0)
    }

    pub fn remove(&mut self, i: usize) {
        if i < self.entries.len() {
            self.entries.remove(i);
            self.hot = None;
            self.stream = None;
        }
    }

    pub fn find_tool(&mut self, id: &str) -> Option<&mut Entry> {
        let i = self
            .entries
            .iter()
            .rposition(|(e, _)| matches!(e, Entry::Tool { id: x, .. } if x == id))?;
        self.get_mut(i)
    }

    pub fn invalidate(&mut self) {
        self.stream = None;
        for (_, cache) in self.entries.iter_mut().chain(&mut self.pins) {
            *cache = None;
        }
    }

    pub(crate) fn lines(&mut self, i: usize, width: usize, p: &Palette) -> &[Line] {
        let n = self.entries.len();
        let (e, cache) = if i < n {
            &mut self.entries[i]
        } else {
            &mut self.pins[i - n]
        };
        if matches!(
            e,
            Entry::Tool {
                timer: Some((_, _, None)),
                ..
            }
        ) || cache.as_ref().is_none_or(|(w, _)| *w != width)
        {
            let lines = match e {
                Entry::Assistant(t) if self.hot == Some(i) && i < n => {
                    crate::md_stream::render(&mut self.stream, i, t, width, p)
                }
                _ => render(e, width, p),
            };
            *cache = Some((width, lines));
        }
        &cache.as_ref().unwrap().1
    }

    /// Returns only the visible `height` lines ending `scroll` lines above the bottom,
    /// plus the clamped `scroll`. Entries are rendered from the bottom up until the
    /// screen is full, so older ones stay unrendered after a resize or a theme change.
    pub fn window(
        &mut self,
        width: usize,
        p: &Palette,
        scroll: usize,
        height: usize,
    ) -> (Vec<Line>, usize) {
        let gap = p.ui.message_gap as usize;
        let scroll = self.apply_anchor(width, p, scroll);
        self.layout.segs.clear();
        let need = scroll + height;
        // Lines from the bottom, newest entry first, with the gaps between entries.
        let mut rev: Vec<Line> = Vec::with_capacity(need);
        for i in (0..self.entries.len() + self.pins.len()).rev() {
            if rev.len() >= need {
                break;
            }
            let lines = self.lines(i, width, p);
            if lines.is_empty() {
                continue;
            }
            let n_lines = lines.len();
            let gap_lines = if rev.is_empty() { 0 } else { gap };
            let take = need - rev.len();
            let mut block: Vec<Line> = lines.iter().rev().take(take).cloned().collect();
            if gap_lines > 0 {
                let mut with_gap: Vec<Line> = (0..gap_lines).map(|_| vec![]).collect();
                with_gap.append(&mut block);
                block = with_gap;
            }
            let slot = self.slot_at(i);
            self.layout
                .segs
                .push((slot, rev.len() + gap_lines, n_lines));
            rev.extend(block);
        }
        rev.truncate(need);
        let clamped = scroll.min(rev.len().saturating_sub(height));
        self.layout.scroll = clamped;
        self.layout.height = height;
        let end = rev.len().min(clamped + height);
        let out = rev[clamped..end].iter().rev().cloned().collect();
        (out, clamped)
    }
}

#[cfg(test)]
#[path = "chat_tests.rs"]
mod tests;
