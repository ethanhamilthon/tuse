use crate::event::Event;
use crate::session::Session;
use std::sync::mpsc::{Receiver, Sender, channel};

pub(crate) enum Subs {
    Pending(Vec<Event>),
    Live(Vec<Sender<Event>>),
}

impl Default for Subs {
    fn default() -> Self {
        Subs::Pending(Vec::new())
    }
}

const PENDING_LIMIT: usize = 64;

impl Session {
    pub(crate) fn emit(&self, e: Event) {
        match &mut *self.subs.lock().unwrap() {
            Subs::Pending(buf) => {
                if buf.len() < PENDING_LIMIT {
                    buf.push(e);
                }
            }
            Subs::Live(subs) => subs.retain(|tx| tx.send(e.clone()).is_ok()),
        }
    }

    pub fn subscribe(&self) -> Receiver<Event> {
        let (tx, rx) = channel();
        let mut subs = self.subs.lock().unwrap();
        match &mut *subs {
            Subs::Pending(buf) => {
                for e in buf.drain(..) {
                    let _ = tx.send(e);
                }
                *subs = Subs::Live(vec![tx]);
            }
            Subs::Live(list) => list.push(tx),
        }
        rx
    }
}
