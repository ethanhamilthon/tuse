use crate::{app::App, picker::Kind};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
impl App {
    /// Returns true when the picker consumed the key. While a modal picker
    /// searches, text editing keys go on to the editor, which is its search box.
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
                    k if k.is_token() => self.dismissed = Some(self.editor.token().1),
                    _ => self.restore_draft(),
                }
            }
            KeyCode::Char('x') if ctrl && p.kind == Kind::Task => {
                if let Some(it) = p.current() {
                    let id = it.value.clone();
                    let _ = self.session.tasks.stop(&id);
                    self.open_tasks();
                }
            }
            _ if p.kind.is_token() => return false,
            _ => return !edits_text(k),
        }
        true
    }
}

/// The keys that edit a one-line search: everything else (Shift+Enter, Ctrl+C,
/// Ctrl+D...) is swallowed so it neither adds a line nor reaches the session.
fn edits_text(k: KeyEvent) -> bool {
    let primary = k
        .modifiers
        .intersects(KeyModifiers::CONTROL | KeyModifiers::SUPER);
    match k.code {
        KeyCode::Char(c) => !primary || matches!(c, 'a' | 'e' | 'u' | 'k' | 'w' | 'v' | 'x'),
        KeyCode::Backspace
        | KeyCode::Delete
        | KeyCode::Left
        | KeyCode::Right
        | KeyCode::Home
        | KeyCode::End => true,
        _ => false,
    }
}
