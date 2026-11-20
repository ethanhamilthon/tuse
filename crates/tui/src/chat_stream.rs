use crate::{app::App, chat::Entry};

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
