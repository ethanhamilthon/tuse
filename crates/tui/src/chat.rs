use crate::render::render;
use crate::style::Palette;
use crate::text::Line;
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
    /// A plain text block from `tuse print`.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn print_blocks_are_replaced_in_place() {
        let mut c = Chat::default();
        assert!(c.set_print("p1", "a".into()));
        c.push(Entry::Notice("n".into()));
        assert!(!c.set_print("p1", "b".into()));
        assert_eq!(c.len(), 2);
        assert!(matches!(c.get_mut(0), Some(Entry::Print { text, .. }) if text == "b"));
    }
}
