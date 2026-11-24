use crate::app::App;
use crate::chat::Entry;
use tuse_core::{Event, UiPrompt};

impl App {
    fn drop_pending(&mut self) {
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
                self.turn_error = None;
            }
            Event::Text(t) => self.stream(t, false),
            Event::Reasoning(t) => self.stream(t, true),
            Event::StreamReset => {
                self.streamed_text = false;
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
                if !self.streamed_text && self.streaming.is_none() && !text.trim().is_empty() {
                    self.chat.push(Entry::Assistant(text));
                }
                self.streaming = None;
                self.thinking = None;
                self.streamed_text = false;
            }
            Event::ToolDelta {
                index, id, code, ..
            } => {
                let pos =
                    if let Some((_, pos)) = self.pending_tools.iter().find(|(i, _)| *i == index) {
                        *pos
                    } else {
                        self.streaming = None;
                        self.thinking = None;
                        let pos = self.chat.len();
                        self.pending_tools.push((index, pos));
                        self.chat.push(Entry::Tool {
                            id: if id.is_empty() {
                                format!("stream-{index}")
                            } else {
                                id
                            },
                            input: String::new(),
                            output: None,
                            code: None,
                            diffs: vec![],
                        });
                        pos
                    };
                if let Some(Entry::Tool { input, .. }) = self.chat.get_mut(pos) {
                    input.push_str(&code);
                }
            }
            Event::ToolStart { id, input, .. } => {
                if let Some((n, _)) = self
                    .pending_tools
                    .iter()
                    .enumerate()
                    .min_by_key(|(_, x)| x.0)
                {
                    let (_, pos) = self.pending_tools.remove(n);
                    if let Some(Entry::Tool {
                        id: old,
                        input: text,
                        ..
                    }) = self.chat.get_mut(pos)
                    {
                        *old = id;
                        *text = input;
                        return;
                    }
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
                }
            }
            Event::Status(_) => {}
            Event::Flash(t) => self.flash(t),
            Event::Notice(t) => self.notice(t),
            Event::Notification(_) => {}
            Event::Error(t) => {
                self.turn_error = Some(t.clone());
                self.drop_pending();
                self.error(t)
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
                if cancelled {
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
                if matches!(&self.picker, Some(p) if p.kind == crate::picker::Kind::Task) {
                    self.open_tasks();
                }
            }
            Event::Usage => {}
            Event::Reloaded => self.restyle(),
            Event::Models => {
                if matches!(&self.picker, Some(p) if p.kind == crate::picker::Kind::Model) {
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
                UiPrompt::Picker { options } => self.open_lua_picker(req.id, options),
                UiPrompt::Question { text } => self.open_question(req.id, text),
            },
            Event::UiClear { id } => {
                if matches!(&self.picker, Some(p) if p.kind == crate::picker::Kind::LuaPicker(id)) {
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
                self.scroll = 0;
            }
        }
    }
}
