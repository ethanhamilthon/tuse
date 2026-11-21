use crate::event::Event;
use crate::session::Session;

impl Session {
    /// Shows `text` in the TUI as a plain block and returns its ID.
    /// With `id`, the text of that earlier block is replaced instead (it keeps the
    /// placement it was created with); empty text removes it and frees the ID.
    /// `pin` keeps a new block at the bottom of the chat.
    pub fn print(&self, text: String, id: Option<String>, pin: bool) -> Result<String, String> {
        self.require_client("draw_ui")?;
        let mut st = self.state.lock().unwrap();
        let ids = &mut st.prints;
        let (id, pin) = match id {
            Some(id) => match ids.get(&id) {
                Some(&pin) => (id, pin),
                None => return Err(format!("no print {id}")),
            },
            None => {
                let mut n = ids.len() + 1;
                while ids.contains_key(&format!("p{n}")) {
                    n += 1;
                }
                let id = format!("p{n}");
                ids.insert(id.clone(), pin);
                (id, pin)
            }
        };
        if text.is_empty() {
            ids.remove(&id);
        }
        // Emitted under the state lock so clients see events in state order.
        self.emit(Event::Print {
            id: id.clone(),
            text,
            pin,
        });
        Ok(id)
    }
}
