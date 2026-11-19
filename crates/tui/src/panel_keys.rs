use crate::app::App;
use crossterm::event::KeyCode;

impl App {
    /// Esc closes the panel (an opened trace item goes back to the list); the scroll keys move
    /// it, in a list they move the selection and Enter opens the item. Returns true when the
    /// key was used.
    pub(crate) fn panel_key(&mut self, code: KeyCode, page: usize) -> bool {
        let step = self.palette.t.scroll_step as isize;
        let page = page as isize;
        let Some(panel) = &mut self.panel else {
            return false;
        };
        match code {
            KeyCode::Esc => {
                if !panel.back() {
                    self.panel = None;
                }
            }
            KeyCode::Up if panel.is_list() => panel.move_by(-1),
            KeyCode::Down if panel.is_list() => panel.move_by(1),
            KeyCode::PageUp if panel.is_list() => panel.move_by(-page),
            KeyCode::PageDown if panel.is_list() => panel.move_by(page),
            KeyCode::Enter if panel.is_list() => panel.open(),
            KeyCode::Up if self.editor.is_empty() => panel.scroll_by(-step),
            KeyCode::Down if self.editor.is_empty() => panel.scroll_by(step),
            KeyCode::PageUp => panel.scroll_by(-page),
            KeyCode::PageDown => panel.scroll_by(page),
            _ => return false,
        }
        true
    }
}
