use crate::event::Event;
use crate::session::Session;

impl Session {
    /// Shows `text` in the TUI as a plain block and returns its ID.
    /// With `id`, the text of that earlier block is replaced instead.
    pub fn print(&self, text: String, id: Option<String>) -> Result<String, String> {
        self.require_client("draw_ui")?;
        let mut st = self.state.lock().unwrap();
        let ids = &mut st.prints;
        let id = match id {
            Some(id) if ids.contains(&id) => id,
            Some(id) => return Err(format!("no print {id}")),
            None => {
                let id = format!("p{}", ids.len() + 1);
                ids.insert(id.clone());
                id
            }
        };
        drop(st);
        self.emit(Event::Print {
            id: id.clone(),
            text,
        });
        Ok(id)
    }
}
