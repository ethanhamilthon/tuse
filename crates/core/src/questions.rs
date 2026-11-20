use crate::{Event, Session};
use std::sync::{
    atomic::Ordering,
    mpsc::{Receiver, channel},
};

impl Session {
    /// Shows a choice in the TUI; the receiver gets the answer.
    pub fn ask(
        &self,
        question: &str,
        options: Vec<String>,
    ) -> Result<(String, Receiver<String>), String> {
        self.require_client("ask")?;
        let id = format!("a{}", self.ask_seq.fetch_add(1, Ordering::Relaxed) + 1);
        let (tx, rx) = channel();
        self.state.lock().unwrap().asks.insert(id.clone(), tx);
        self.emit(Event::Ask {
            id: id.clone(),
            question: question.to_string(),
            options,
        });
        Ok((id, rx))
    }

    /// Whether the ask `id` is still waiting for an answer.
    pub fn has_ask(&self, id: &str) -> bool {
        self.state.lock().unwrap().asks.contains_key(id)
    }

    /// Answers an ask; `None` dismisses it.
    pub fn answer(&self, id: &str, answer: Option<String>) {
        let tx = self.state.lock().unwrap().asks.remove(id);
        if let Some(tx) = tx
            && let Some(a) = answer
        {
            let _ = tx.send(a);
        }
        self.emit(Event::AskDone { id: id.to_string() });
    }
}
