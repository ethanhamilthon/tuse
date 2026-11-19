use crate::event::Event;
use crate::session::Session;

impl Session {
    /// Shows `text` in the TUI as a plain block and returns its ID.
    /// With `id`, the text of that earlier block is replaced instead.
    pub fn print(&self, text: String, id: Option<String>) -> Result<String, String> {
        if self.headless {
            return Err("no TUI is attached to this session (headless)".into());
        }
        let mut ids = self.prints.lock().unwrap();
        let id = match id {
            Some(id) if ids.contains(&id) => id,
            Some(id) => return Err(format!("no print {id}")),
            None => {
                let id = format!("p{}", ids.len() + 1);
                ids.insert(id.clone());
                id
            }
        };
        drop(ids);
        self.emit(Event::Print {
            id: id.clone(),
            text,
        });
        Ok(id)
    }
}
