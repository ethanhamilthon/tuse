use crate::chat::{Chat, Entry};
use tuse_core::Session;

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
        let config = session.config();
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
                chat.push_record(r, &|m| config.display_name(m));
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
