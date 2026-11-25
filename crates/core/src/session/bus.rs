use crate::convo::record::Record;
use crate::event::{Event, UiRequest};
use crate::session::{Session, State};
use std::sync::mpsc::{Receiver, Sender, SyncSender, channel};

pub(crate) enum Subs {
    Pending(Vec<Event>),
    Live(Vec<Subscriber>),
}

pub(crate) struct Subscriber {
    tx: Sender<Event>,
    wake: Option<SyncSender<()>>,
}

impl Default for Subs {
    fn default() -> Self {
        Subs::Pending(Vec::new())
    }
}

const PENDING_LIMIT: usize = 64;

pub(crate) enum Signal {
    TurnStart,
    Text(String),
    Reasoning(String),
    ToolDelta {
        index: usize,
        id: String,
        name: String,
        code: String,
    },
    ToolStart {
        timeout: u64,
        id: String,
        tool: String,
        input: String,
    },
    Status(String),
    Tasks,
    Usage,
    Models,
    Reloaded,
    HistoryReplaced,
    UiRequest(UiRequest),
    UiClear {
        id: u64,
    },
}

impl Session {
    pub(crate) fn record(
        &self,
        st: &mut State,
        records: &[Record],
        events: impl IntoIterator<Item = Event>,
    ) -> bool {
        if let Err(e) = self.write_history(st, None, None, records, &self.config()) {
            if e != "session closed" {
                eprintln!("tuse: history: {e}");
            }
            return false;
        }
        for e in events {
            self.publish(e);
        }
        true
    }

    pub(crate) fn emit(&self, signal: Signal) {
        self.publish(match signal {
            Signal::TurnStart => Event::TurnStart,
            Signal::Text(value) => Event::Text(value),
            Signal::Reasoning(value) => Event::Reasoning(value),
            Signal::ToolDelta {
                index,
                id,
                name,
                code,
            } => Event::ToolDelta {
                index,
                id,
                name,
                code,
            },
            Signal::ToolStart {
                timeout,
                id,
                tool,
                input,
            } => Event::ToolStart {
                timeout,
                id,
                tool,
                input,
            },
            Signal::Status(value) => Event::Status(value),
            Signal::Tasks => Event::Tasks,
            Signal::Usage => Event::Usage,
            Signal::Models => Event::Models,
            Signal::Reloaded => Event::Reloaded,
            Signal::HistoryReplaced => Event::HistoryReplaced,
            Signal::UiRequest(value) => Event::UiRequest(value),
            Signal::UiClear { id } => Event::UiClear { id },
        });
    }

    fn publish(&self, e: Event) {
        match &mut *self.subs.lock().unwrap() {
            Subs::Pending(buf) => {
                if buf.len() < PENDING_LIMIT {
                    buf.push(e);
                }
            }
            Subs::Live(subs) => subs.retain(|sub| {
                if sub.tx.send(e.clone()).is_err() {
                    return false;
                }
                if let Some(wake) = &sub.wake {
                    let _ = wake.try_send(());
                }
                true
            }),
        }
    }

    pub(crate) fn wake_ui(&self) {
        if let Subs::Live(subs) = &*self.subs.lock().unwrap() {
            for sub in subs {
                if let Some(wake) = &sub.wake {
                    let _ = wake.try_send(());
                }
            }
        }
    }

    pub fn subscribe(&self) -> Receiver<Event> {
        self.subscribe_inner(None)
    }

    pub fn subscribe_ui(&self, wake: SyncSender<()>) -> Receiver<Event> {
        self.subscribe_inner(Some(wake))
    }

    fn subscribe_inner(&self, wake: Option<SyncSender<()>>) -> Receiver<Event> {
        let (tx, rx) = channel();
        let sub = Subscriber { tx, wake };
        let mut subs = self.subs.lock().unwrap();
        match &mut *subs {
            Subs::Pending(buf) => {
                for e in buf.drain(..) {
                    let _ = sub.tx.send(e);
                }
                *subs = Subs::Live(vec![sub]);
            }
            Subs::Live(list) => list.push(sub),
        }
        rx
    }
}
