use crate::kit::entry::Entry;
use crate::kit::rich::render::render;
use crate::kit::style::Palette;
use crate::kit::text::Line;
use std::time::Duration;
use tuse_core::{Record, Session};

type Cache = Option<(usize, Vec<Line>)>;

#[derive(Default)]
pub struct Chat {
    pub(crate) entries: Vec<(Entry, Cache)>,
    pub(crate) pinned: Vec<(String, String, Cache)>,
    hot: Option<usize>,
    pub(crate) tick: usize,
    stream: Option<crate::kit::rich::md_stream::Prefix>,
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
            Record::UiMarkdown {
                name,
                text,
                pin,
                removed,
            } => {
                self.set_markdown(name.clone(), text.clone(), *pin, *removed);
                return;
            }
            Record::UiResult {
                kind,
                prompt,
                value,
                ..
            } => Entry::UiResult {
                is_picker: kind == "picker",
                prompt: prompt.clone(),
                value: value.clone(),
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

    pub(crate) fn finish_interrupted_tools(&mut self) {
        for (entry, cache) in self.entries.iter_mut().rev() {
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

    pub fn invalidate(&mut self) {
        self.stream = None;
        for (_, cache) in self.entries.iter_mut() {
            *cache = None;
        }
        for (_, _, cache) in self.pinned.iter_mut() {
            *cache = None;
        }
    }

    pub(crate) fn lines(&mut self, i: usize, width: usize, p: &Palette) -> &[Line] {
        let (e, cache) = &mut self.entries[i];
        if cache.as_ref().is_none_or(|(w, _)| *w != width) {
            let lines = match e {
                Entry::Assistant(t) if self.hot == Some(i) => {
                    crate::kit::rich::md_stream::render(&mut self.stream, i, t, width, p)
                }
                _ => render(e, width, p),
            };
            *cache = Some((width, lines));
        }
        let (_, lines) = cache.as_mut().unwrap();
        if matches!(e, Entry::Tool { output: None, .. })
            && let Some(prefix) = lines.first_mut().and_then(|line| line.first_mut())
        {
            const STARS: [&str; 4] = ["✦ ", "✧ ", "✦ ", "✶ "];
            let frame = STARS[self.tick % STARS.len()];
            let rest: String = prefix.text.chars().skip(2).collect();
            prefix.text = format!("{frame}{rest}");
        }
        lines
    }

    pub fn total(&mut self, width: usize, p: &Palette) -> usize {
        let gap = p.ui.message_gap as usize;
        let mut n = 0;
        let mut parts: usize = 0;
        for i in 0..self.pinned.len() {
            let (_, text, cache) = &mut self.pinned[i];
            if cache.as_ref().is_none_or(|(w, _)| *w != width) {
                *cache = Some((width, crate::kit::rich::render::ui_markdown(text, width, p)));
            }
            let len = cache.as_ref().unwrap().1.len();
            if len > 0 {
                n += len;
                parts += 1;
            }
        }
        for i in 0..self.entries.len() {
            let len = self.lines(i, width, p).len();
            if len > 0 {
                n += len;
                parts += 1;
            }
        }
        n + gap * parts.saturating_sub(1)
    }

    pub fn window(
        &mut self,
        width: usize,
        p: &Palette,
        scroll: usize,
        height: usize,
    ) -> (Vec<Line>, usize) {
        let gap = p.ui.message_gap as usize;
        let need = scroll + height;
        let mut rev: Vec<Line> = Vec::with_capacity(need);
        for i in (0..self.pinned.len()).rev() {
            if rev.len() >= need {
                break;
            }
            let (_, text, cache) = &mut self.pinned[i];
            if cache.as_ref().is_none_or(|(w, _)| *w != width) {
                *cache = Some((width, crate::kit::rich::render::ui_markdown(text, width, p)));
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
        let clamped = scroll.min(rev.len().saturating_sub(height));
        let end = rev.len().min(clamped + height);
        let out = rev.drain(clamped..end).rev().collect();
        (out, clamped)
    }
}

fn push_rev(rev: &mut Vec<Line>, lines: &[Line], need: usize, gap: usize) {
    if lines.is_empty() {
        return;
    }
    if !rev.is_empty() {
        rev.resize_with(rev.len() + gap.min(need - rev.len()), Vec::new);
    }
    rev.extend(lines.iter().rev().take(need - rev.len()).cloned());
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
    fn replays_persisted_ui_records_in_position_with_pin_and_update() {
        let user = |t: &str| Record::User {
            text: t.into(),
            context: String::new(),
            images: vec![],
            at: String::new(),
        };
        let md = |name: &str, text: &str, pin: bool, removed: bool| Record::UiMarkdown {
            name: name.into(),
            text: text.into(),
            pin,
            removed,
        };
        let c = Chat::from_records(
            &[
                md("a", "v1", false, false),
                md("p", "pinned-1", true, false),
                md("tmp", "drop", false, false),
                user("turn 1"),
                Record::UiResult {
                    id: 1,
                    kind: "picker".into(),
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
            .map(|(e, _)| match e {
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
                Record::Notice {
                    text: "notice".into(),
                },
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
        assert_eq!(app.chat.lines(n, 40, &app.palette)[0][0].text, "✧ ");
        app.handle_event(Event::ToolEnd {
            id: "id".into(),
            output: "1".into(),
            code: Some(0),
            diffs: vec![],
        });
        assert_eq!(app.chat.lines(n, 40, &app.palette)[0][0].text, "* ");
        crate::ui::app::cleanup(app);
    }

    #[test]
    fn window_slices_and_invalidate_clears_cache() {
        let mut p = Palette::new(&Default::default(), &Default::default());
        p.ui.message_gap = 4;
        let mut c = Chat::default();
        c.push(Entry::Notice("one".into()));
        c.push(Entry::Notice("two".into()));
        c.push(Entry::Notice("three".into()));
        let (rows, scroll) = c.window(40, &p, 0, 2);
        assert_eq!(scroll, 0);
        assert_eq!(rows.len(), 2);
        c.invalidate();
        assert!(c.entries.iter().all(|(_, cache)| cache.is_none()));
        c.push(Entry::Assistant("a\n\nb\n\nc".into()));
        c.push(Entry::Reasoning(String::new()));
        c.push(Entry::Error("e".into()));
        let gap = p.ui.message_gap as usize;
        let mut all: Vec<Line> = vec![];
        for (e, _) in &c.entries {
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
