use crate::app::App;
use crate::chat::Entry;
use tuse_core::Event;

impl App {
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
            Event::TurnStart => {}
            Event::Text(t) => self.stream(t, false),
            Event::Reasoning(t) => self.stream(t, true),
            Event::StreamReset => {
                let mut drop: Vec<usize> = [self.streaming.take(), self.thinking.take()]
                    .into_iter()
                    .flatten()
                    .collect();
                drop.sort_unstable_by(|a, b| b.cmp(a));
                for i in drop {
                    self.chat.remove(i);
                }
            }
            Event::Assistant { text } => {
                if self.streaming.is_none() && !text.trim().is_empty() {
                    self.chat.push(Entry::Assistant(text));
                }
                self.streaming = None;
                self.thinking = None;
            }
            Event::ToolStart { id, input, .. } => {
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
            Event::Notice(t) | Event::Notification(t) => self.notice(t),
            Event::Error(t) => self.error(t),
            Event::TurnEnd {
                elapsed,
                calls,
                cancelled,
            } => {
                self.streaming = None;
                self.thinking = None;
                self.chat.push(Entry::TurnEnd {
                    elapsed,
                    calls,
                    cancelled,
                });
            }
            Event::Ask {
                id,
                question,
                options,
            } => {
                self.asks.push((id, question, options));
                if self.picker.as_ref().is_none_or(|p| p.kind.is_token()) {
                    self.next_ask();
                }
            }
            Event::AskDone { id } => {
                self.asks.retain(|a| a.0 != id);
                if matches!(&self.picker, Some(p) if p.kind == crate::picker::Kind::Ask(id.clone()))
                {
                    self.picker = None;
                    self.next_ask();
                }
            }
            Event::Tasks => {
                if matches!(&self.picker, Some(p) if p.kind == crate::picker::Kind::Task) {
                    self.open_tasks();
                }
            }
            Event::Usage => {}
            Event::Print { id, text } => {
                if self.chat.set_print(&id, text) {
                    self.scroll = 0;
                }
            }
            Event::Models => {
                if matches!(&self.picker, Some(p) if p.kind == crate::picker::Kind::Model) {
                    self.open_models();
                }
            }
        }
    }

    fn stream(&mut self, t: String, reasoning: bool) {
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
