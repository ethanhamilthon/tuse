use crate::kit::entry::Entry;
use crate::kit::rich::render::{
    chat_inset, message, message_width, pad_message, render, ui_markdown,
};
use crate::kit::selection::HitRow;
use crate::kit::style::Palette;
use crate::kit::text::{Line, MessageRow};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tuse_core::{Level, Record, Session};

type Cache = Option<(usize, Vec<MessageRow>)>;
#[derive(Default)]
struct Stream {
    message: Option<crate::kit::rich::stream::Prefix<MessageRow>>,
    decoration: Option<crate::kit::rich::stream::Prefix<Line>>,
}
static NEXT_ENTRY: AtomicU64 = AtomicU64::new(0);

#[derive(Default)]
pub struct ChatRow {
    pub line: Line,
    pub hit: Option<HitRow>,
}
const STREAMS: usize = 8;

#[derive(Default)]
pub struct Chat {
    pub(crate) entries: Vec<(Entry, Cache, u64)>,
    pub(crate) pinned: Vec<(String, String, Cache, u64)>,
    pub(crate) tick: usize,
    streams: Vec<(usize, Stream)>,
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
            Record::Agent {
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
            Record::Lua {
                id,
                code,
                output,
                exit,
                diffs,
                ..
            } => Entry::Tool {
                id: id.clone(),
                input: code.clone(),
                output: Some(output.clone()),
                code: *exit,
                diffs: diffs.clone(),
            },
            Record::Message {
                level: Level::Notice,
                text,
            } => Entry::Notice(text.clone()),
            Record::Message {
                level: Level::Error,
                text,
            } => Entry::Error(text.clone()),
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
            Record::Show { name, text, pin } => {
                self.set_markdown(name.clone(), text.clone(), *pin, text.is_empty() && !pin);
                return;
            }
            Record::Pick { prompt, value, .. } | Record::Ask { prompt, value, .. } => {
                Entry::UiResult {
                    is_picker: matches!(r, Record::Pick { .. }),
                    prompt: prompt.clone(),
                    value: value.clone(),
                }
            }
            _ => return,
        };
        self.push(e);
    }

    pub fn push(&mut self, e: Entry) {
        self.entries
            .push((e, None, NEXT_ENTRY.fetch_add(1, Ordering::Relaxed)));
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub(crate) fn entry(&self, i: usize) -> Option<&Entry> {
        self.entries.get(i).map(|(entry, _, _)| entry)
    }

    pub fn get_mut(&mut self, i: usize) -> Option<&mut Entry> {
        let slot = self.entries.get_mut(i)?;
        slot.1 = None;
        let live = matches!(
            slot.0,
            Entry::Assistant(_) | Entry::Reasoning(_) | Entry::Tool { output: None, .. }
        );
        if live && !self.streams.iter().any(|(n, _)| *n == i) {
            if self.streams.len() >= STREAMS {
                self.streams.remove(0);
            }
            self.streams.push((i, Stream::default()));
        }
        Some(&mut slot.0)
    }

    pub fn remove(&mut self, i: usize) {
        if i < self.entries.len() {
            self.entries.remove(i);
            self.streams.clear();
        }
    }

    pub fn find_tool(&mut self, id: &str) -> Option<&mut Entry> {
        let i = self
            .entries
            .iter()
            .rposition(|(e, _, _)| matches!(e, Entry::Tool { id: x, .. } if x == id))?;
        self.get_mut(i)
    }

    pub(crate) fn finish_interrupted_tools(&mut self) {
        for (entry, cache, _) in self.entries.iter_mut().rev() {
            if matches!(entry, Entry::TurnEnd { .. }) {
                break;
            }
            if let Entry::Tool { output, .. } = entry
                && output.is_none()
            {
                *output = Some("[interrupted: no output]".into());
                *cache = None;
            }
        }
    }

    pub(crate) fn settle(&mut self) {
        for (i, _) in std::mem::take(&mut self.streams) {
            if let Some(slot) = self.entries.get_mut(i) {
                slot.1 = None;
            }
        }
    }

    pub fn invalidate(&mut self) {
        for (_, s) in &mut self.streams {
            *s = Stream::default();
        }
        for (_, cache, _) in self.entries.iter_mut() {
            *cache = None;
        }
        for (_, _, cache, _) in self.pinned.iter_mut() {
            *cache = None;
        }
    }

    fn rows(&mut self, i: usize, width: usize, p: &Palette) -> &[MessageRow] {
        let (e, cache, _) = &mut self.entries[i];
        if cache.as_ref().is_none_or(|(w, _)| *w != width) {
            use crate::kit::rich::stream;
            let live = self.streams.iter_mut().find(|(n, _)| *n == i);
            let lines = match (&*e, live) {
                (Entry::Assistant(t), Some((_, s))) => {
                    let mut lines = stream::markdown(&mut s.message, t, message_width(width), p);
                    for l in &mut lines {
                        pad_message(l, width, Default::default());
                    }
                    lines
                }
                (Entry::Reasoning(t), Some((_, s)))
                    if p.ui.lines.reasoning < 0 && !t.trim().is_empty() =>
                {
                    decorations(stream::reasoning(&mut s.decoration, t, width, p))
                }
                (
                    Entry::Tool {
                        input,
                        output: None,
                        ..
                    },
                    Some((_, s)),
                ) => decorations(stream::tool(&mut s.decoration, input, width, p)),
                (
                    Entry::User { .. }
                    | Entry::Assistant(_)
                    | Entry::Markdown { .. }
                    | Entry::UiResult { .. },
                    _,
                ) => message(e, width, p),
                (e, _) => decorations(render(e, width, p)),
            };
            *cache = Some((width, lines));
        }
        let (_, lines) = cache.as_mut().unwrap();
        if matches!(e, Entry::Tool { output: None, .. })
            && chat_inset(width) > 0
            && let Some(prefix) = lines.first_mut().and_then(|row| row.line.first_mut())
            && let Some(mark) = prefix.text.chars().next()
        {
            let stars = crate::kit::style::TOOL_STARS;
            let frame = stars[self.tick % stars.len()];
            prefix.text.replace_range(..mark.len_utf8(), frame);
        }
        lines
    }

    #[cfg(test)]
    fn lines(&mut self, i: usize, width: usize, p: &Palette) -> Vec<Line> {
        self.rows(i, width, p)
            .iter()
            .map(|row| row.line.clone())
            .collect()
    }

    pub fn total(&mut self, width: usize, p: &Palette) -> usize {
        let mut parts: Vec<(bool, usize)> = vec![];
        for i in 0..self.entries.len() {
            let message = is_message(&self.entries[i].0);
            parts.push((message, self.rows(i, width, p).len()));
        }
        for (_, text, cache, _) in &mut self.pinned {
            if cache.as_ref().is_none_or(|(w, _)| *w != width) {
                *cache = Some((width, ui_markdown(text, width, p)));
            }
            parts.push((false, cache.as_ref().unwrap().1.len()));
        }
        let mut n = 0;
        let mut above: Option<bool> = None;
        for (message, len) in parts.into_iter().filter(|(_, len)| *len > 0) {
            n += len + above.map_or(0, |a| gap_between(p, a, message));
            above = Some(message);
        }
        n
    }

    pub fn viewport(
        &mut self,
        width: usize,
        p: &Palette,
        scroll: usize,
        height: usize,
    ) -> (Vec<ChatRow>, usize) {
        let need = scroll + height;
        let mut rev: Vec<ChatRow> = Vec::with_capacity(need);
        let mut below: Option<bool> = None;
        for i in (0..self.pinned.len()).rev() {
            if rev.len() >= need {
                break;
            }
            let (_, text, cache, id) = &mut self.pinned[i];
            if cache.as_ref().is_none_or(|(w, _)| *w != width) {
                *cache = Some((width, ui_markdown(text, width, p)));
            }
            let lines = &cache.as_ref().unwrap().1;
            let gap = below.map_or(0, |b| gap_between(p, b, false));
            push_rev(&mut rev, lines, *id, need, gap);
            if !lines.is_empty() {
                below = Some(false);
            }
        }
        for i in (0..self.entries.len()).rev() {
            if rev.len() >= need {
                break;
            }
            let id = self.entries[i].2;
            let message = is_message(&self.entries[i].0);
            let rows = self.rows(i, width, p);
            let gap = below.map_or(0, |b| gap_between(p, b, message));
            push_rev(&mut rev, rows, id, need, gap);
            if !rows.is_empty() {
                below = Some(message);
            }
        }
        let clamped = scroll.min(rev.len().saturating_sub(height));
        let end = rev.len().min(clamped + height);
        let out = rev.drain(clamped..end).rev().collect();
        (out, clamped)
    }

    #[cfg(test)]
    pub fn window(
        &mut self,
        width: usize,
        p: &Palette,
        scroll: usize,
        height: usize,
    ) -> (Vec<Line>, usize) {
        let (rows, clamped) = self.viewport(width, p, scroll, height);
        (rows.into_iter().map(|row| row.line).collect(), clamped)
    }
}

fn is_message(e: &Entry) -> bool {
    matches!(e, Entry::User { .. } | Entry::Assistant(_))
}

fn gap_between(p: &Palette, a: bool, b: bool) -> usize {
    usize::from(if a || b {
        p.ui.space.messages
    } else {
        p.ui.space.steps
    })
}

fn decorations(lines: Vec<Line>) -> Vec<MessageRow> {
    lines.into_iter().map(MessageRow::decoration).collect()
}

fn push_rev(rev: &mut Vec<ChatRow>, lines: &[MessageRow], id: u64, need: usize, gap: usize) {
    if lines.is_empty() {
        return;
    }
    if !rev.is_empty() {
        rev.resize_with(rev.len() + gap.min(need - rev.len()), ChatRow::default);
    }
    rev.extend(
        lines
            .iter()
            .enumerate()
            .rev()
            .take(need - rev.len())
            .map(|(index, row)| ChatRow {
                line: row.line.clone(),
                hit: row.copy.clone().map(|copy| HitRow { id, index, copy }),
            }),
    );
}

impl Chat {
    pub fn from_session(session: &Session) -> Chat {
        let mut chat = Chat::default();
        session.with_records(|records| {
            for r in records {
                chat.push_record(r, &|m| session.display_name(m));
            }
        });
        chat
    }

    pub(crate) fn markdown_id(&self, name: &str) -> Option<u64> {
        self.pinned
            .iter()
            .find(|(n, _, _, _)| n == name)
            .map(|slot| slot.3)
            .or_else(|| {
                self.entries.iter().find_map(|(entry, _, id)| {
                    matches!(entry, Entry::Markdown { name: n, .. } if n == name).then_some(*id)
                })
            })
    }

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
                .position(|(e, _, _)| matches!(e, Entry::Markdown { name: n, .. } if n == name))?;
            chat.remove(pos);
            Some(pos)
        };
        if removed {
            self.pinned.retain(|(n, _, _, _)| n != &name);
            return remove_entry(self, &name);
        }
        if pin {
            let shifted = remove_entry(self, &name);
            if let Some(slot) = self.pinned.iter_mut().find(|(n, _, _, _)| n == &name) {
                slot.1 = text;
                slot.2 = None;
            } else {
                self.pinned
                    .push((name, text, None, NEXT_ENTRY.fetch_add(1, Ordering::Relaxed)));
            }
            return shifted;
        }
        self.pinned.retain(|(n, _, _, _)| n != &name);
        if let Some(pos) = self
            .entries
            .iter()
            .position(|(e, _, _)| matches!(e, Entry::Markdown { name: n, .. } if n == &name))
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
    fn replays_persisted_ui_records_in_position_with_pin_and_update() {
        let user = |t: &str| Record::User {
            text: t.into(),
            context: String::new(),
            images: vec![],
        };
        let md = |name: &str, text: &str, pin: bool, _: bool| Record::Show {
            name: name.into(),
            text: text.into(),
            pin,
        };
        let c = Chat::from_records(
            &[
                md("a", "v1", false, false),
                md("p", "pinned-1", true, false),
                md("tmp", "drop", false, false),
                user("turn 1"),
                Record::Pick {
                    id: 1,
                    prompt: "Picker".into(),
                    value: "optA".into(),
                },
                md("a", "v2", false, false),
                md("tmp", "", false, true),
                md("p", "unpinned", false, false),
                user("turn 2"),
                md("end_pin", "stay-end", true, false),
            ],
            &str::to_owned,
        );
        let tags: Vec<String> = c
            .entries
            .iter()
            .map(|(e, _, _)| match e {
                Entry::User { text, .. } => format!("u:{text}"),
                Entry::Markdown { name, text } => format!("m:{name}={text}"),
                Entry::UiResult {
                    is_picker, value, ..
                } => format!("r:{is_picker}:{value}"),
                _ => "other".into(),
            })
            .collect();
        assert_eq!(
            tags,
            [
                "m:a=v2",
                "u:turn 1",
                "r:true:optA",
                "m:p=unpinned",
                "u:turn 2"
            ]
        );
        assert_eq!(c.pinned.len(), 1);
        assert_eq!(
            (c.pinned[0].0.as_str(), c.pinned[0].1.as_str()),
            ("end_pin", "stay-end")
        );
    }

    #[test]
    fn notifications_are_not_chat_entries() {
        let c = Chat::from_records(
            &[
                Record::Notification {
                    text: "ready".into(),
                },
                Record::message(Level::Notice, "notice"),
            ],
            &str::to_owned,
        );
        assert_eq!(c.len(), 1);
        assert!(matches!(&c.entries[0].0, Entry::Notice(t) if t == "notice"));
        let mut app = crate::ui::app::test_app("notification-chat");
        let n = app.chat.len();
        app.handle_event(tuse_core::Event::Notification("ready".into()));
        assert_eq!(app.chat.len(), n);
        app.handle_event(tuse_core::Event::Notice("notice".into()));
        assert!(matches!(&app.chat.entries[n].0, Entry::Notice(t) if t == "notice"));
        crate::ui::app::cleanup(app);
    }

    #[test]
    fn empty_lua_previews_do_not_take_a_row() {
        use tuse_core::Event;
        let mut app = crate::ui::app::test_app("empty-tool-chat");
        let n = app.chat.len();
        let delta = |code: &str| Event::ToolDelta {
            index: 0,
            id: "id".into(),
            name: "lua".into(),
            code: code.into(),
        };
        app.handle_event(delta(""));
        app.handle_event(delta(" \n"));
        assert_eq!(app.chat.len(), n);
        app.handle_event(Event::StreamReset);
        assert!(app.hidden_tool_code.is_empty());
        app.handle_event(delta(""));
        app.handle_event(Event::ToolStart {
            timeout: 120,
            id: "id".into(),
            tool: "lua".into(),
            input: "print(1)".into(),
        });
        assert_eq!(app.chat.len(), n + 1);
        let line = &app.chat.lines(n, 40, &app.palette)[0];
        assert_eq!(
            line.iter().map(|s| s.text.as_str()).collect::<String>(),
            "✦ print(1)"
        );
        crate::ui::app::cleanup(app);
    }

    #[test]
    fn tool_deltas_fill_the_existing_tool_entry() {
        use tuse_core::Event;
        let mut app = crate::ui::app::test_app("tool-chat");
        let n = app.chat.len();
        app.handle_event(Event::ToolDelta {
            index: 0,
            id: "id".into(),
            name: "lua".into(),
            code: "print(1)".into(),
        });
        let text =
            |l: &crate::kit::text::Line| l.iter().map(|s| s.text.as_str()).collect::<String>();
        assert_eq!(text(&app.chat.lines(n, 40, &app.palette)[0]), "✦ print(1)");
        app.handle_event(Event::ToolStart {
            timeout: 120,
            id: "id".into(),
            tool: "lua".into(),
            input: "1".into(),
        });
        assert_eq!(app.chat.len(), n + 1);
        assert!(matches!(&app.chat.entries[n].0, Entry::Tool { input, .. } if input == "1"));
        app.chat.tick = 1;
        assert!(
            app.chat.lines(n, 40, &app.palette)[0][0]
                .text
                .starts_with("✧ ")
        );
        app.handle_event(Event::ToolEnd {
            id: "id".into(),
            output: "1".into(),
            code: Some(0),
            diffs: vec![],
        });
        assert!(
            app.chat.lines(n, 40, &app.palette)[0][0]
                .text
                .starts_with("* ")
        );
        crate::ui::app::cleanup(app);
    }

    #[test]
    fn restored_chat_aligns_all_entry_types_and_fits_narrow_widths() {
        use crate::kit::text::{line_width, width};
        let settings = tuse_config::Ui {
            lines: tuse_config::Lines {
                reasoning: -1,
                diff: -1,
                ..Default::default()
            },
            ..Default::default()
        };
        let p = Palette::new(&settings);
        let records = [
            Record::User {
                text: "first\nsecond".into(),
                context: String::new(),
                images: vec![],
            },
            Record::Agent {
                text: "first\n\nsecond".into(),
                reasoning: "thought".into(),
                tool_calls: vec![],
                replay: None,
                cancelled: false,
            },
            Record::Lua {
                id: "t".into(),
                code: "return 1".into(),
                output: "output".into(),
                exit: Some(0),
                images: vec![],
                diffs: vec![tuse_core::FileDiff {
                    path: "file".into(),
                    diff: "+line".into(),
                }],
            },
            Record::message(Level::Notice, "note"),
            Record::message(Level::Error, "error"),
            Record::TurnEnd {
                elapsed_ms: 1000,
                cancelled: false,
                model: "m".into(),
                effort: "e".into(),
                calls: 0,
            },
            Record::Pick {
                id: 1,
                prompt: "prompt".into(),
                value: "answer".into(),
            },
            Record::Show {
                name: "a".into(),
                text: "unpinned".into(),
                pin: false,
            },
            Record::Show {
                name: "b".into(),
                text: "pinned".into(),
                pin: true,
            },
        ];
        let mut chat = Chat::from_records(&records, &str::to_owned);
        for w in 1..=25 {
            let (rows, _) = chat.window(w, &p, 0, 512);
            assert!(rows.iter().all(|r| line_width(r) <= w), "width {w}");
        }
        let (rows, _) = chat.window(25, &p, 0, 60);
        for want in [
            "first", "second", "thought", "return 1", "output", "file", "+line", "note", "error",
            "1s", "prompt", "answer", "unpinned", "pinned",
        ] {
            assert!(
                rows.iter()
                    .map(|r| r.iter().map(|s| s.text.as_str()).collect::<String>())
                    .any(|r| r.find(want).is_some_and(|at| width(&r[..at]) == 2)),
                "missing or misaligned {want}"
            );
        }
        for i in [0, 2] {
            let rows = chat.lines(i, 8, &p);
            for row in rows
                .iter()
                .filter(|r| !r.iter().all(|span| span.text.trim().is_empty()))
            {
                let text = row.iter().map(|s| s.text.as_str()).collect::<String>();
                assert!(text.starts_with("  ") && text.ends_with("  "), "{text:?}");
            }
        }
    }

    #[test]
    fn steps_and_messages_have_their_own_gaps() {
        let total = |steps: u16, messages: u16| {
            let mut p = Palette::new(&Default::default());
            p.ui.space.steps = steps;
            p.ui.space.messages = messages;
            let mut c = Chat::default();
            c.push(Entry::Notice("a".into()));
            c.push(Entry::Notice("b".into()));
            c.push(Entry::Assistant("c".into()));
            let total = c.total(40, &p);
            assert_eq!(c.window(40, &p, 0, 100).0.len(), total);
            total
        };
        let base = total(0, 0);
        assert_eq!(total(3, 0), base + 3);
        assert_eq!(total(0, 2), base + 2);
    }

    #[test]
    fn window_slices_and_invalidate_clears_cache() {
        let mut p = Palette::new(&Default::default());
        p.ui.space.steps = 4;
        p.ui.space.messages = 4;
        let mut c = Chat::default();
        c.push(Entry::Notice("one".into()));
        c.push(Entry::Notice("two".into()));
        c.push(Entry::Notice("three".into()));
        let (rows, scroll) = c.window(40, &p, 0, 2);
        assert_eq!(scroll, 0);
        assert_eq!(rows.len(), 2);
        c.invalidate();
        assert!(c.entries.iter().all(|(_, cache, _)| cache.is_none()));
        c.push(Entry::Assistant("a\n\nb\n\nc".into()));
        c.push(Entry::Reasoning(String::new()));
        c.push(Entry::Error("e".into()));
        let gap = p.ui.space.steps as usize;
        let mut all: Vec<Line> = vec![];
        for (e, _, _) in &c.entries {
            let lines = crate::kit::rich::render::render(e, 40, &p);
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
        c.invalidate();
        c.window(40, &p, 0, 1);
        assert!(c.entries[0].1.is_none());
    }

    #[test]
    #[ignore]
    fn bench_stream() {
        let p = Palette::new(&Default::default());
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
        let p = Palette::new(&Default::default());
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
