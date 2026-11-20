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
        id: String,
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
    entries: Vec<(Entry, Cache)>,
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
                    input,
                    output,
                    code,
                    diffs,
                    ..
                } => Entry::Tool {
                    id: id.clone(),
                    input: input.clone(),
                    output: Some(output.clone()),
                    code: *code,
                    diffs: diffs.clone(),
                },
                Record::Notice { text } | Record::Notification { text } => {
                    Entry::Notice(text.clone())
                }
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

    /// Replaces the text of the print block `id`, or adds a new block (returns true).
    pub fn set_print(&mut self, id: &str, text: String) -> bool {
        let at = self
            .entries
            .iter()
            .rposition(|(e, _)| matches!(e, Entry::Print { id: x, .. } if x == id));
        match at.and_then(|i| self.get_mut(i)) {
            Some(Entry::Print { text: old, .. }) => {
                *old = text;
                false
            }
            _ => {
                self.push(Entry::Print {
                    id: id.to_string(),
                    text,
                });
                true
            }
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
        for (_, cache) in &mut self.entries {
            *cache = None;
        }
    }

    /// Returns only the visible `height` lines ending `scroll` lines above the bottom,
    /// plus the clamped `scroll`, without cloning off-screen entries.
    pub fn window(
        &mut self,
        width: usize,
        p: &Palette,
        scroll: usize,
        height: usize,
    ) -> (Vec<Line>, usize) {
        let gap = p.ui.message_gap as usize;
        let mut total = 0usize;
        let mut non_empty = false;
        for (i, (e, cache)) in self.entries.iter_mut().enumerate() {
            if cache.as_ref().is_none_or(|(w, _)| *w != width) {
                let lines = match e {
                    Entry::Assistant(t) if self.hot == Some(i) => {
                        crate::md_stream::render(&mut self.stream, i, t, width, p)
                    }
                    _ => render(e, width, p),
                };
                *cache = Some((width, lines));
            }
            let len = cache.as_ref().unwrap().1.len();
            if len == 0 {
                continue;
            }
            if non_empty {
                total += gap;
            }
            total += len;
            non_empty = true;
        }
        let clamped = scroll.min(total.saturating_sub(height));
        let end = total - clamped;
        let start = end.saturating_sub(height);
        let mut out = Vec::with_capacity(end - start);
        let mut pos = 0usize;
        let mut seen = false;
        for (_, cache) in &self.entries {
            let lines = &cache.as_ref().unwrap().1;
            if lines.is_empty() {
                continue;
            }
            if seen && gap > 0 {
                let g_start = pos.max(start);
                let g_end = (pos + gap).min(end);
                if g_start < g_end {
                    out.extend((g_start..g_end).map(|_| vec![]));
                }
                pos += gap;
            }
            let l_start = pos.max(start);
            let l_end = (pos + lines.len()).min(end);
            if l_start < l_end {
                out.extend(lines[l_start - pos..l_end - pos].iter().cloned());
            }
            pos += lines.len();
            seen = true;
            if pos >= end {
                break;
            }
        }
        (out, clamped)
    }
}

#[cfg(test)]
#[path = "chat_tests.rs"]
mod tests;
