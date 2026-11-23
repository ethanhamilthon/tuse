use crate::app::App;
use crossterm::event::KeyCode;

impl App {
    pub(crate) fn panel_key(&mut self, code: KeyCode, page: usize) -> bool {
        let Some(panel) = &mut self.panel else {
            return false;
        };
        let step = self.palette.ui.scroll_step as isize;
        match code {
            KeyCode::Esc => self.panel = None,
            KeyCode::Up if self.editor.is_empty() => panel.scroll_by(-step),
            KeyCode::Down if self.editor.is_empty() => panel.scroll_by(step),
            KeyCode::PageUp => panel.scroll_by(-(page as isize)),
            KeyCode::PageDown => panel.scroll_by(page as isize),
            _ => return false,
        }
        true
    }
}
