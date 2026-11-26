use crate::kit::entry::Entry;
use crate::ui::app::App;
use crate::ui::app::Pump;
use tuse_core::{Event, UiPrompt};

impl App {
    pub fn pump(&mut self, last_tick: &mut std::time::Instant) -> Pump {
        let mut pump = Pump::default();
        for i in 0..256 {
            let Ok(e) = self.events.try_recv() else { break };
            pump.more = i == 255;
            if matches!(e, Event::Status(_)) {
                continue;
            }
            let visible = match &e {
                Event::Text(t) => Some(!t.trim().is_empty()),
                Event::Reasoning(t) => {
                    Some(self.palette.ui.lines.reasoning != 0 && !t.trim().is_empty())
                }
                Event::ToolDelta { code, .. } => {
                    Some(self.palette.ui.lines.tool != 0 && !code.trim().is_empty())
                }
                _ => None,
            };
            pump.immediate |= match visible {
                None => true,
                Some(true) => !std::mem::replace(&mut self.visible_stream, true),
                Some(false) => false,
            };
            self.handle_event(e);
            pump.dirty = true;
        }
        let activity = self.session.activity();
        if self.activity != activity {
            self.activity = activity;
            pump.dirty = true;
            pump.immediate = true;
        }
        if activity == tuse_core::Activity::Provider && self.has_retry_notice() {
            self.flash = None;
            pump.dirty = true;
        }
        if std::mem::take(&mut self.refresh_history) {
            self.chat = crate::ui::chat::Chat::from_session(&self.session);
            self.streaming = None;
            self.thinking = None;
            self.pending_tools.clear();
            self.hidden_tool_code.clear();
            self.streamed_text = false;
        }
        if !self.retry_notice_active()
            && self.flash.as_ref().is_some_and(|(_, since)| {
                since.elapsed() >= std::time::Duration::from_secs(crate::kit::style::FLASH_SECS)
            })
        {
            self.flash = None;
            pump.dirty = true;
        }
        let tasks = self.session.running_tasks();
        if std::mem::replace(&mut self.tasks_shown, tasks) != tasks {
            pump.dirty = true;
            pump.immediate = true;
        }
        if (self.session.busy() || tasks > 0)
            && last_tick.elapsed() >= std::time::Duration::from_millis(100)
        {
            self.tick += 1;
            self.chat.tick = self.tick;
            *last_tick = std::time::Instant::now();
            pump.dirty = true;
            pump.immediate = true;
        }
        pump
    }

    pub(crate) fn retry_notice_active(&self) -> bool {
        self.activity == tuse_core::Activity::Retry && self.has_retry_notice()
    }

    fn has_retry_notice(&self) -> bool {
        self.flash
            .as_ref()
            .is_some_and(|(text, _)| text.starts_with("Retry "))
    }

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
    fn drop_pending(&mut self) {
        self.hidden_tool_code.clear();
        self.pending_tools.sort_by_key(|(_, pos)| *pos);
        for (_, pos) in self.pending_tools.drain(..).rev() {
            self.chat.remove(pos);
            for idx in [&mut self.streaming, &mut self.thinking]
                .into_iter()
                .flatten()
            {
                if *idx > pos {
                    *idx -= 1;
                }
            }
        }
    }
    pub fn handle_event(&mut self, e: Event) {
        match e {
            Event::User { id, text, images } => {
                if self.steering == Some(id) {
                    self.editor.clear();
                    self.steering = None;
                }
                self.chat.push(Entry::User { text, images });
                self.scroll = 0;
            }
            Event::TurnStart => {
                self.visible_stream = false;
                self.turn_error = None;
            }
            Event::Text(t) => self.stream(t, false),
            Event::Reasoning(t) => self.stream(t, true),
            Event::StreamReset => {
                self.visible_stream = false;
                self.streamed_text = false;
                self.hidden_tool_code.clear();
                let mut drop: Vec<usize> = [self.streaming.take(), self.thinking.take()]
                    .into_iter()
                    .flatten()
                    .collect();
                drop.extend(self.pending_tools.drain(..).map(|(_, pos)| pos));
                drop.sort_unstable_by(|a, b| b.cmp(a));
                for i in drop {
                    self.chat.remove(i);
                }
            }
            Event::Assistant { text } => {
                self.hidden_tool_code.clear();
                self.visible_stream = false;
                if !self.streamed_text && self.streaming.is_none() && !text.trim().is_empty() {
                    self.chat.push(Entry::Assistant(text));
                }
                self.streaming = None;
                self.thinking = None;
                self.streamed_text = false;
                self.chat.settle();
            }
            Event::ToolDelta {
                index,
                id,
                mut code,
                ..
            } => {
                let pos =
                    if let Some((_, pos)) = self.pending_tools.iter().find(|(i, _)| *i == index) {
                        *pos
                    } else {
                        if code.trim().is_empty() {
                            if !code.is_empty() {
                                self.hidden_tool_code
                                    .entry(index)
                                    .or_default()
                                    .push_str(&code);
                            }
                            return;
                        }
                        if let Some(mut prefix) = self.hidden_tool_code.remove(&index) {
                            prefix.push_str(&code);
                            code = prefix;
                        }
                        self.streaming = None;
                        self.thinking = None;
                        let pos = self.chat.len();
                        self.pending_tools.push((index, pos));
                        self.chat.push(Entry::Tool {
                            id: if id.is_empty() {
                                format!("stream-{index}")
                            } else {
                                id.clone()
                            },
                            input: String::new(),
                            output: None,
                            code: None,
                            diffs: vec![],
                        });
                        pos
                    };
                if let Some(Entry::Tool { id: old, input, .. }) = self.chat.get_mut(pos) {
                    if !id.is_empty() {
                        *old = id;
                    }
                    input.push_str(&code);
                }
            }
            Event::ToolStart { id, input, .. } => {
                let slot = self.pending_tools.iter().position(|&(_, pos)| {
                    matches!(self.chat.entry(pos), Some(Entry::Tool { id: old, .. }) if old == &id)
                }).or_else(|| {
                    self.pending_tools.iter().enumerate().filter(|&(_, &(index, pos))| {
                        matches!(self.chat.entry(pos), Some(Entry::Tool { id: old, .. }) if old == &format!("stream-{index}"))
                    }).min_by_key(|(_, entry)| entry.0).map(|(n, _)| n)
                });
                if let Some(n) = slot {
                    let (_, pos) = self.pending_tools.remove(n);
                    if let Some(Entry::Tool {
                        id: old,
                        input: text,
                        ..
                    }) = self.chat.get_mut(pos)
                    {
                        *old = id;
                        if !input.trim().is_empty() {
                            *text = input;
                        }
                        return;
                    }
                }
                if input.trim().is_empty() {
                    self.empty_tool = Some(id);
                    return;
                }
                self.chat.push(Entry::Tool {
                    id,
                    input,
                    output: None,
                    code: None,
                    diffs: vec![],
                });
            }
            Event::ToolEnd {
                id,
                output,
                code,
                diffs,
            } => {
                let handed_off = code.is_none() && output.starts_with("[continued in background: ");
                if let Some(Entry::Tool {
                    output: o,
                    code: c,
                    diffs: d,
                    ..
                }) = self.chat.find_tool(&id)
                {
                    *o = Some(output);
                    *c = code;
                    *d = diffs;
                } else if self.empty_tool.as_ref() == Some(&id) {
                    self.empty_tool = None;
                    self.chat.push(Entry::Tool {
                        id,
                        input: String::new(),
                        output: Some(output),
                        code,
                        diffs,
                    });
                }
                if handed_off {
                    self.drop_pending();
                }
            }
            Event::Status(_) => {}
            Event::Flash(t) => self.display_flash(t),
            Event::Notice(t) => self.chat.push(Entry::Notice(t)),
            Event::Notification(_) => {}
            Event::Error(t) => {
                self.turn_error = Some(t.clone());
                self.drop_pending();
                self.chat.push(Entry::Error(t));
            }
            Event::TurnEnd {
                elapsed,
                model,
                effort,
                cancelled,
                ..
            } => {
                self.streaming = None;
                self.thinking = None;
                self.drop_pending();
                self.empty_tool = None;
                self.visible_stream = false;
                if self
                    .flash
                    .as_ref()
                    .is_some_and(|(text, _)| text.starts_with("Retry "))
                {
                    self.flash = None;
                }
                self.chat.settle();
                if cancelled {
                    self.chat.finish_interrupted_tools();
                    self.turn_error = None;
                }
                self.chat.push(Entry::TurnEnd {
                    elapsed,
                    model: self.session.display_name(&model),
                    effort,
                    cancelled,
                });
            }
            Event::Tasks => {
                if matches!(&self.picker, Some(p) if p.kind == crate::ui::picker::Kind::Task) {
                    self.open_tasks();
                }
            }
            Event::Usage => {}
            Event::Reloaded => self.restyle(),
            Event::HistoryReplaced => {
                self.refresh_history = true;
            }
            Event::Switch(tuse_core::Switch::Fork(index)) => self.fork(index),
            Event::Switch(tuse_core::Switch::Resume(id)) => self.switch(Some(id)),
            Event::Models => {
                if matches!(&self.picker, Some(p) if p.kind == crate::ui::picker::Kind::Model) {
                    self.open_models();
                }
            }
            Event::UiMarkdown {
                name,
                text,
                pin,
                removed,
            } => {
                if let Some(pos) = self.chat.set_markdown(name, text, pin, removed) {
                    for idx in [&mut self.streaming, &mut self.thinking]
                        .into_iter()
                        .flatten()
                    {
                        if *idx > pos {
                            *idx -= 1;
                        }
                    }
                    for (_, p) in &mut self.pending_tools {
                        if *p > pos {
                            *p -= 1;
                        }
                    }
                }
            }
            Event::UiRequest(req) => match req.prompt {
                UiPrompt::Picker { options, title } => self.open_lua_picker(req.id, options, title),
                UiPrompt::Question { text } => self.open_question(req.id, text),
            },
            Event::UiClear { id } => {
                if matches!(&self.picker, Some(p) if p.kind == crate::ui::picker::Kind::LuaPicker(id))
                {
                    self.close_modal();
                }
                if self.question.as_ref().is_some_and(|q| q.id == id) {
                    self.question = None;
                    self.restore_draft();
                }
            }
            Event::UiResult(entry) => {
                self.chat.push(Entry::UiResult {
                    is_picker: entry.kind == "picker",
                    prompt: entry.prompt,
                    value: entry.value,
                });
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    #[test]
    fn burst_drains_in_order_in_bounded_batches() {
        let mut app = crate::ui::app::test_app("stream-burst");
        let (tx, rx) = std::sync::mpsc::channel();
        app.events = rx;
        for n in 0..300 {
            tx.send(Event::Text(format!("{n} "))).unwrap();
        }
        let mut tick = Instant::now();
        let first = app.pump(&mut tick);
        assert!(first.dirty && first.immediate && first.more);
        let second = app.pump(&mut tick);
        assert!(second.dirty && !second.immediate && !second.more);
        let expected: String = (0..300).map(|n| format!("{n} ")).collect();
        assert!(
            matches!(app.chat.entry(app.streaming.unwrap()), Some(Entry::Assistant(t)) if t == &expected)
        );
        crate::ui::app::cleanup(app);
    }
}
