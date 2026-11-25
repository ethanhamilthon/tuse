use crate::convo::record::Record;
use crate::event::{Event, UiEntry, UiMarkdown, UiPrompt, UiRequest, UiSnapshot};
use crate::session::Session;
use crate::session::bus::Signal;
use std::collections::VecDeque;
use std::sync::mpsc;

pub type UiReply = Result<serde_json::Value, String>;

struct PendingReq {
    req: UiRequest,
    is_hook: bool,
    tx: mpsc::Sender<UiReply>,
}

#[derive(Default)]
pub(crate) struct UiState {
    next_id: u64,
    closed: bool,
    deny_hooks: bool,
    queue: VecDeque<PendingReq>,
}

#[derive(Default)]
struct UiHistory {
    markdowns: Vec<UiMarkdown>,
    entries: Vec<UiEntry>,
}

impl UiHistory {
    pub(crate) fn apply_markdown(&mut self, name: String, text: String, pin: bool, removed: bool) {
        if removed {
            self.markdowns.retain(|b| b.name != name);
            return;
        }
        let block = UiMarkdown {
            name: name.clone(),
            text: text.clone(),
            pin,
        };
        match self.markdowns.iter().position(|b| b.name == name) {
            Some(i) if self.markdowns[i].pin == pin => self.markdowns[i].text = text,
            Some(i) => {
                self.markdowns.remove(i);
                self.markdowns.push(block);
            }
            None => self.markdowns.push(block),
        }
    }

    fn read(records: &[Record]) -> Self {
        let mut ui = Self::default();
        for r in records {
            match r {
                Record::UiMarkdown {
                    name,
                    text,
                    pin,
                    removed,
                } => ui.apply_markdown(name.clone(), text.clone(), *pin, *removed),
                Record::UiResult {
                    id,
                    kind,
                    prompt,
                    value,
                } => {
                    let kind = if kind == "picker" {
                        "picker"
                    } else {
                        "question"
                    };
                    ui.entries.push(UiEntry {
                        id: *id,
                        kind,
                        prompt: prompt.clone(),
                        value: value.clone(),
                    });
                }
                _ => {}
            }
        }
        ui
    }
}

impl Session {
    pub fn ui_snapshot(&self) -> UiSnapshot {
        let ui = self.ui.lock().unwrap();
        let history = self.with_records(UiHistory::read);
        UiSnapshot {
            markdowns: history.markdowns,
            active: ui.queue.front().map(|p| p.req.clone()),
            entries: history.entries,
        }
    }

    pub(crate) fn ui_markdown(&self, name: String, text: String, pin: bool) {
        let removed = text.is_empty() && !pin;
        self.message(
            Record::UiMarkdown {
                name: name.clone(),
                text: text.clone(),
                pin,
                removed,
            },
            Event::UiMarkdown {
                name,
                text,
                pin,
                removed,
            },
        );
    }

    pub(crate) fn ui_enqueue(
        &self,
        prompt: UiPrompt,
        is_hook: bool,
    ) -> Result<(u64, mpsc::Receiver<UiReply>), String> {
        let (req, rx, active) = {
            let mut ui = self.ui.lock().unwrap();
            if ui.closed {
                return Err("session closed".into());
            }
            if is_hook && ui.deny_hooks {
                return Err("interactive UI is unavailable during reload or close".into());
            }
            let last = self.with_records(|rs| {
                rs.iter()
                    .filter_map(|r| match r {
                        Record::UiResult { id, .. } => Some(*id),
                        _ => None,
                    })
                    .max()
                    .unwrap_or(0)
            });
            ui.next_id = ui.next_id.max(last) + 1;
            let req = UiRequest {
                id: ui.next_id,
                prompt,
            };
            let (tx, rx) = mpsc::channel();
            let active = ui.queue.is_empty();
            ui.queue.push_back(PendingReq {
                req: req.clone(),
                is_hook,
                tx,
            });
            (req, rx, active)
        };
        let id = req.id;
        if active {
            self.emit(Signal::UiRequest(req));
        }
        Ok((id, rx))
    }

    pub fn ui_reply(&self, id: u64, value: Option<String>) -> Result<(), String> {
        let next = {
            let mut ui = self.ui.lock().unwrap();
            let Some(front) = ui.queue.front() else {
                return Err(format!("no active UI request {id}"));
            };
            if front.req.id != id {
                return Err(format!("UI request {id} is not active"));
            }
            if let (Some(v), UiPrompt::Picker { options }) = (&value, &front.req.prompt)
                && !options.contains(v)
            {
                return Err(format!("invalid picker option `{v}`"));
            }
            let pending = ui.queue.pop_front().unwrap();
            if let Some(v) = &value {
                let (kind, prompt) = match pending.req.prompt {
                    UiPrompt::Picker { .. } => ("picker", "Picker".into()),
                    UiPrompt::Question { text } => ("question", text),
                };
                let e = UiEntry {
                    id,
                    kind,
                    prompt: prompt.clone(),
                    value: v.clone(),
                };
                self.message(
                    Record::UiResult {
                        id,
                        kind: kind.into(),
                        prompt,
                        value: v.clone(),
                    },
                    Event::UiResult(e),
                );
            }
            let reply = value.map_or(serde_json::Value::Null, serde_json::Value::String);
            let _ = pending.tx.send(Ok(reply));
            ui.queue.front().map(|p| p.req.clone())
        };
        if let Some(req) = next {
            self.emit(Signal::UiRequest(req));
        }
        Ok(())
    }

    pub(crate) fn ui_cancel_request(&self, id: u64) {
        let (cleared, next) = {
            let mut ui = self.ui.lock().unwrap();
            let Some(pos) = ui.queue.iter().position(|p| p.req.id == id) else {
                return;
            };
            ui.queue.remove(pos);
            let next = (pos == 0)
                .then(|| ui.queue.front().map(|p| p.req.clone()))
                .flatten();
            (pos == 0, next)
        };
        if cleared {
            self.emit(Signal::UiClear { id });
            if let Some(req) = next {
                self.emit(Signal::UiRequest(req));
            }
        }
    }

    pub(crate) fn ui_cancel_hooks(&self, deny: bool) {
        let (cleared, next) = {
            let mut ui = self.ui.lock().unwrap();
            ui.deny_hooks = deny;
            if !deny {
                return;
            }
            let old = ui.queue.front().filter(|p| p.is_hook).map(|p| p.req.id);
            let mut keep = VecDeque::new();
            for p in ui.queue.drain(..) {
                if p.is_hook {
                    let _ = p.tx.send(Err("cancelled".into()));
                } else {
                    keep.push_back(p);
                }
            }
            ui.queue = keep;
            (
                old,
                old.and_then(|_| ui.queue.front().map(|p| p.req.clone())),
            )
        };
        if let Some(id) = cleared {
            self.emit(Signal::UiClear { id });
            if let Some(req) = next {
                self.emit(Signal::UiRequest(req));
            }
        }
    }

    pub(crate) fn ui_cancel_all(&self) {
        let active = {
            let mut ui = self.ui.lock().unwrap();
            let active = ui.queue.front().map(|p| p.req.id);
            for p in ui.queue.drain(..) {
                let _ = p.tx.send(Err("cancelled".into()));
            }
            active
        };
        if let Some(id) = active {
            self.emit(Signal::UiClear { id });
        }
    }

    pub(crate) fn ui_close(&self) {
        let cleared = {
            let mut ui = self.ui.lock().unwrap();
            ui.closed = true;
            ui.deny_hooks = true;
            let active = ui.queue.front().map(|p| p.req.id);
            for p in ui.queue.drain(..) {
                let _ = p.tx.send(Err("session closed".into()));
            }
            active
        };
        if let Some(id) = cleared {
            self.emit(Signal::UiClear { id });
        }
    }
}
