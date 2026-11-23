use crate::app::App;
use crossterm::event::KeyCode;

impl App {
    pub(crate) fn panel_key(&mut self, code: KeyCode) -> bool {
        let Some(panel) = &mut self.panel else {
            return false;
        };
        let step = self.palette.ui.scroll_step as isize;
        match code {
            KeyCode::Esc => {
                self.panel = None;
                if let Some((picker, search)) = self.trace_back.take() {
                    self.picker = Some(picker);
                    self.editor = search;
                }
            }
            KeyCode::Up if self.editor.is_empty() => panel.scroll_by(-step),
            KeyCode::Down if self.editor.is_empty() => panel.scroll_by(step),
            _ => return false,
        }
        true
    }
}
