use crate::app::App;
use crate::render::render;
use crate::style::Palette;
use crate::text::Line;
use std::time::Duration;
use tuse_core::{FileDiff, Record, Session};

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

impl App {
    pub(super) fn stream(&mut self, t: String, reasoning: bool) {
        let slot = if reasoning {
            self.thinking
        } else {
            self.streaming
        };
        if let Some(i) = slot
            && let Some(Entry::Assistant(s) | Entry::Reasoning(s)) = self.chat.get_mut(i)
        {
            s.push_str(&t);
            return;
        }
        if !reasoning {
            self.streamed_text = true;
        }
        let i = self.chat.len();
        self.chat.push(if reasoning {
            Entry::Reasoning(t)
        } else {
            Entry::Assistant(t)
        });
        if reasoning {
            self.thinking = Some(i);
        } else {
            self.streaming = Some(i);
        }
    }
}

enum Transient {
    Markdown {
        name: String,
        text: String,
    },
    Result {
        is_picker: bool,
        prompt: String,
        value: String,
    },
}

impl Chat {
    /// Builds chat history plus live transient UI items (kept across `/reload`, not resume).
    pub fn from_session(session: &Session) -> Chat {
        let snap = session.ui_snapshot();
        let mut transient: Vec<(usize, u64, Transient)> = vec![];
        let mut pinned = vec![];
        for m in snap.markdowns {
            if m.pin {
                pinned.push((m.name, m.text, None));
            } else {
                transient.push((
                    m.after_record,
                    m.seq,
                    Transient::Markdown {
                        name: m.name,
                        text: m.text,
                    },
                ));
            }
        }
        for e in snap.entries {
            transient.push((
                e.after_record,
                e.seq,
                Transient::Result {
                    is_picker: e.kind == "picker",
                    prompt: e.prompt,
                    value: e.value,
                },
            ));
        }
        transient.sort_by_key(|(at, seq, _)| (*at, *seq));
        let mut chat = Chat::default();
        let mut ti = 0usize;
        session.with_records(|records| {
            let flush = |chat: &mut Chat, ti: &mut usize, upto: usize| {
                while *ti < transient.len() && transient[*ti].0 <= upto {
                    let entry = match &transient[*ti].2 {
                        Transient::Markdown { name, text } => Entry::Markdown {
                            name: name.clone(),
                            text: text.clone(),
                        },
                        Transient::Result {
                            is_picker,
                            prompt,
                            value,
                        } => Entry::UiResult {
                            is_picker: *is_picker,
                            prompt: prompt.clone(),
                            value: value.clone(),
                        },
                    };
                    chat.push(entry);
                    *ti += 1;
                }
            };
            flush(&mut chat, &mut ti, 0);
            for (i, r) in records.iter().enumerate() {
                chat.push_record(r, &|m| session.display_name(m));
                flush(&mut chat, &mut ti, i + 1);
            }
            flush(&mut chat, &mut ti, usize::MAX);
        });
        chat.pinned = pinned;
        chat
    }

    /// Creates, updates, moves (pin/unpin), or removes a named Markdown block.
    /// Returns the removed index in `entries` when an unpinned entry is removed.
    pub fn set_markdown(
        &mut self,
        name: String,
        text: String,
        pin: bool,
        removed: bool,
    ) -> Option<usize> {
        let remove_entry = |chat: &mut Chat, name: &str| -> Option<usize> {
            let pos = chat
                .entries
                .iter()
                .position(|(e, _)| matches!(e, Entry::Markdown { name: n, .. } if n == name))?;
            chat.remove(pos);
            Some(pos)
        };
        if removed {
            self.pinned.retain(|(n, _, _)| n != &name);
            return remove_entry(self, &name);
        }
        if pin {
            let shifted = remove_entry(self, &name);
            if let Some(slot) = self.pinned.iter_mut().find(|(n, _, _)| n == &name) {
                slot.1 = text;
                slot.2 = None;
            } else {
                self.pinned.push((name, text, None));
            }
            return shifted;
        }
        self.pinned.retain(|(n, _, _)| n != &name);
        if let Some(pos) = self
            .entries
            .iter()
            .position(|(e, _)| matches!(e, Entry::Markdown { name: n, .. } if n == &name))
        {
            if let Some(Entry::Markdown { text: t, .. }) = self.get_mut(pos) {
                *t = text;
            }
            None
        } else {
            self.push(Entry::Markdown { name, text });
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn notifications_are_not_chat_entries() {
        let c = Chat::from_records(
            &[
                Record::Notification {
                    text: "ready".into(),
                },
                Record::Notice {
                    text: "notice".into(),
                },
            ],
            &str::to_owned,
        );
        assert_eq!(c.len(), 1);
        assert!(matches!(&c.entries[0].0, Entry::Notice(t) if t == "notice"));
        let mut app = crate::app::test_app("notification-chat");
        let n = app.chat.len();
        app.handle_event(tuse_core::Event::Notification("ready".into()));
        assert_eq!(app.chat.len(), n);
        app.handle_event(tuse_core::Event::Notice("notice".into()));
        assert!(matches!(&app.chat.entries[n].0, Entry::Notice(t) if t == "notice"));
        crate::app::cleanup(app);
    }

    #[test]
    fn tool_deltas_fill_the_existing_tool_entry() {
        use tuse_core::Event;
        let mut app = crate::app::test_app("tool-chat");
        let n = app.chat.len();
        app.handle_event(Event::ToolDelta {
            index: 0,
            id: "id".into(),
            name: "lua".into(),
            code: "1".into(),
        });
        app.handle_event(Event::ToolStart {
            timeout: 120,
            id: "id".into(),
            tool: "lua".into(),
            input: "1".into(),
        });
        assert_eq!(app.chat.len(), n + 1);
        assert!(matches!(&app.chat.entries[n].0, Entry::Tool { input, .. } if input == "1"));
        crate::app::cleanup(app);
    }

    #[test]
    fn window_slices_and_invalidate_clears_cache() {
        let p = Palette::new(&Default::default(), &Default::default());
        let mut c = Chat::default();
        c.push(Entry::Notice("one".into()));
        c.push(Entry::Notice("two".into()));
        c.push(Entry::Notice("three".into()));
        let (rows, scroll) = c.window(40, &p, 0, 2);
        assert_eq!(scroll, 0);
        assert_eq!(rows.len(), 2);
        c.invalidate();
        assert!(c.entries.iter().all(|(_, cache)| cache.is_none()));
        // Bottom-up rendering gives the same rows as slicing the whole chat.
        c.push(Entry::Assistant("a\n\nb\n\nc".into()));
        c.push(Entry::Reasoning(String::new()));
        c.push(Entry::Error("e".into()));
        let gap = p.ui.message_gap as usize;
        let mut all: Vec<Line> = vec![];
        for (e, _) in &c.entries {
            let lines = crate::render::render(e, 40, &p);
            if lines.is_empty() {
                continue;
            }
            if !all.is_empty() {
                all.extend((0..gap).map(|_| vec![]));
            }
            all.extend(lines);
        }
        for scroll in 0..all.len() + 3 {
            let (rows, clamped) = c.window(40, &p, scroll, 3);
            let s = clamped.min(all.len() - 3);
            assert_eq!(clamped, scroll.min(all.len() - 3));
            assert_eq!(
                rows,
                all[all.len() - 3 - s..all.len() - s],
                "scroll {scroll}"
            );
        }
        // Older entries stay unrendered when the screen is full.
        c.invalidate();
        c.window(40, &p, 0, 1);
        assert!(c.entries[0].1.is_none());
    }

    #[test]
    #[ignore]
    fn bench_stream() {
        let p = Palette::new(&Default::default(), &Default::default());
        let block = "## Section\n\nSome **bold** text with `code` and a [link](x). More words here to wrap around.\n\n| a | b |\n|---|---|\n| 1 | 2 |\n| 3 | 4 |\n\n```rust\nfn main() {\n    println!(\"hi\");\n}\n```\n\n- one\n- two\n\n";
        let full = block.repeat(30000 / block.len() + 1);
        let mut c = Chat::default();
        c.push(Entry::Assistant(String::new()));
        let start = std::time::Instant::now();
        let mut frames = 0;
        for chunk in full.as_bytes().chunks(40) {
            if let Some(Entry::Assistant(s)) = c.get_mut(0) {
                s.push_str(std::str::from_utf8(chunk).unwrap());
            }
            c.window(100, &p, 0, 50);
            frames += 1;
        }
        eprintln!(
            "BENCH {} bytes, {frames} frames, {:?}",
            full.len(),
            start.elapsed()
        );
    }

    #[test]
    #[ignore]
    fn bench_resize() {
        let p = Palette::new(&Default::default(), &Default::default());
        let mut c = Chat::default();
        for i in 0..2000 {
            c.push(Entry::User {
                text: format!("question {i}"),
                images: 0,
            });
            c.push(Entry::Assistant("## Answer\n\nSome **bold** text and `code`.\n\n```rust\nfn x() {}\n```\n\n| a | b |\n|---|---|\n| 1 | 2 |\n".into()));
        }
        let start = std::time::Instant::now();
        for w in 60..80 {
            c.window(w, &p, 0, 50);
        }
        eprintln!("BENCH resize x20 over 4000 entries: {:?}", start.elapsed());
    }
}
