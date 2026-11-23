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
    Markdown {
        name: String,
        text: String,
    },
    UiResult {
        is_picker: bool,
        prompt: String,
        value: String,
    },
}

/// Wrapped lines of an entry and the width they were wrapped to.
type Cache = Option<(usize, Vec<Line>)>;

/// Chat entries with their wrapped lines cached per width.
#[derive(Default)]
pub struct Chat {
    pub(crate) entries: Vec<(Entry, Cache)>,
    /// Pinned `show` blocks rendered together at the end of the chat.
    pub(crate) pinned: Vec<(String, String, Cache)>,
    /// The reply that is changing (streaming) and its rendered finished blocks.
    hot: Option<usize>,
    stream: Option<crate::md_stream::Prefix>,
}

impl Chat {
    #[cfg(test)]
    pub fn from_records(records: &[Record], display: &dyn Fn(&str) -> String) -> Chat {
        let mut c = Chat::default();
        for r in records {
            c.push_record(r, display);
        }
        c
    }

    pub(crate) fn push_record(&mut self, r: &Record, display: &dyn Fn(&str) -> String) {
        let e = match r {
            Record::User { text, images, .. } => Entry::User {
                text: text.clone(),
                images: images.len(),
            },
            Record::Assistant {
                text, reasoning, ..
            } => {
                if !reasoning.trim().is_empty() {
                    self.push(Entry::Reasoning(reasoning.clone()));
                }
                if text.trim().is_empty() {
                    return;
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
            _ => return,
        };
        self.push(e);
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

    #[cfg(test)]
    pub fn invalidate(&mut self) {
        self.stream = None;
        for (_, cache) in self.entries.iter_mut() {
            *cache = None;
        }
    }

    pub(crate) fn lines(&mut self, i: usize, width: usize, p: &Palette) -> &[Line] {
        let (e, cache) = &mut self.entries[i];
        if cache.as_ref().is_none_or(|(w, _)| *w != width) {
            let lines = match e {
                Entry::Assistant(t) if self.hot == Some(i) => {
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
        let need = scroll + height;
        // Lines from the bottom: pinned blocks last in chat (so first here), then entries.
        let mut rev: Vec<Line> = Vec::with_capacity(need);
        for i in (0..self.pinned.len()).rev() {
            if rev.len() >= need {
                break;
            }
            let (_, text, cache) = &mut self.pinned[i];
            if cache.as_ref().is_none_or(|(w, _)| *w != width) {
                *cache = Some((width, crate::render::ui_markdown(text, width, p)));
            }
            push_rev(&mut rev, &cache.as_ref().unwrap().1, need, gap);
        }
        for i in (0..self.entries.len()).rev() {
            if rev.len() >= need {
                break;
            }
            let lines = self.lines(i, width, p);
            push_rev(&mut rev, lines, need, gap);
        }
        rev.truncate(need);
        let clamped = scroll.min(rev.len().saturating_sub(height));
        let end = rev.len().min(clamped + height);
        let out = rev[clamped..end].iter().rev().cloned().collect();
        (out, clamped)
    }
}

fn push_rev(rev: &mut Vec<Line>, lines: &[Line], need: usize, gap: usize) {
    if lines.is_empty() {
        return;
    }
    let gap_lines = if rev.is_empty() { 0 } else { gap };
    let take = need - rev.len();
    let mut block: Vec<Line> = lines.iter().rev().take(take).cloned().collect();
    if gap_lines > 0 {
        let mut with_gap: Vec<Line> = (0..gap_lines).map(|_| vec![]).collect();
        with_gap.append(&mut block);
        block = with_gap;
    }
    rev.extend(block);
}

#[cfg(test)]
#[path = "chat_tests.rs"]
mod tests;
