use crate::{app::App, picker::Kind};
use crossterm::event::{KeyCode, KeyEvent};
impl App {
    /// Returns true when the picker consumed the key.
    pub(crate) fn picker_key(&mut self, k: KeyEvent, ctrl: bool, shift: bool) -> bool {
        let rows = self.picker_rows();
        let Some(p) = &mut self.picker else {
            return false;
        };
        match k.code {
            KeyCode::Up | KeyCode::Down if shift && p.kind.is_token() => return false,
            KeyCode::Up => p.moved(-1, rows),
            KeyCode::Down => p.moved(1, rows),
            KeyCode::Char('p') if ctrl => p.moved(-1, rows),
            KeyCode::Char('n') if ctrl => p.moved(1, rows),
            KeyCode::Enter | KeyCode::Tab if !shift => self.choose(),
            KeyCode::Esc => {
                let p = self.picker.take().unwrap();
                match &p.kind {
                    Kind::Ask(id) => {
                        let id = id.clone();
                        self.session.answer(&id, None);
                        self.asks.retain(|a| a.0 != id);
                        self.next_ask();
                    }
                    k if k.is_token() => self.dismissed = Some(self.editor.token().1),
                    _ => {}
                }
            }
            KeyCode::Char('x') if ctrl && p.kind == Kind::Task => {
                if let Some(it) = p.current() {
                    let id = it.value.clone();
                    let _ = self.session.tasks.stop(&id);
                    self.open_tasks();
                }
            }
            _ if matches!(p.kind, Kind::Ask(_)) => {}
            _ if p.kind.is_token() => return false,
            KeyCode::Char(c) if !ctrl => {
                let q = format!("{}{c}", p.query);
                p.filter(&q);
            }
            KeyCode::Backspace => {
                let mut q = p.query.clone();
                q.pop();
                p.filter(&q);
            }
            _ => {}
        }
        true
    }
}
