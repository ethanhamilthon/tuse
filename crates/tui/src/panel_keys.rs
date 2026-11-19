use crate::app::App;
use crossterm::event::KeyCode;

impl App {
    /// Esc closes the panel (an opened trace item goes back to the list); the scroll keys
    /// move it. Returns true when the key was used.
    pub(crate) fn panel_key(&mut self, code: KeyCode, page: usize) -> bool {
        let step = self.palette.t.scroll_step as isize;
        let page = page as isize;
        let Some(panel) = &mut self.panel else {
            return false;
        };
        match code {
            KeyCode::Esc => match self.trace_item {
                Some(at) => self.open_trace(Some(at)),
                None => self.panel = None,
            },
            KeyCode::Up if self.editor.is_empty() => panel.scroll_by(-step),
            KeyCode::Down if self.editor.is_empty() => panel.scroll_by(step),
            KeyCode::PageUp => panel.scroll_by(-page),
            KeyCode::PageDown => panel.scroll_by(page),
            _ => return false,
        }
        true
    }
}
