use crate::app::App;
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

impl App {
    pub fn picker_rows(&self) -> usize {
        self.list_rows.max(1)
    }

    pub fn key(&mut self, mut k: KeyEvent, page: usize) {
        let page = page.max(1);
        if k.kind == KeyEventKind::Release {
            return;
        }
        if k.modifiers
            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER)
            && let KeyCode::Char(c) = k.code
            && let Some(mapped) = crate::layout_keys::latin(c)
        {
            k.code = KeyCode::Char(mapped);
        }
        let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
        let alt = k.modifiers.contains(KeyModifiers::ALT);
        let shift = k.modifiers.contains(KeyModifiers::SHIFT);
        let primary = ctrl || k.modifiers.contains(KeyModifiers::SUPER);
        if primary
            && k.code == KeyCode::Char('c')
            && (!self.selection.text.is_empty() || self.editor.selection_range().is_some())
        {
            self.copy_selection(false);
            return;
        }
        self.selection = Default::default();
        if self.picker.is_some() && self.picker_key(k, ctrl, shift) {
            return;
        }
        if self.question_key(k.code, shift || alt, page) {
            return;
        }
        if self.panel_key(k.code, page) {
            return;
        }
        let before = self.steering.map(|_| self.editor.buf.clone());
        let cols = self.input_cols;
        let e = &mut self.editor;
        if matches!(
            k.code,
            KeyCode::Left
                | KeyCode::Right
                | KeyCode::Up
                | KeyCode::Down
                | KeyCode::Home
                | KeyCode::End
        ) {
            if shift {
                e.anchor.get_or_insert(e.cursor);
            } else {
                e.clear_selection();
            }
        } else if matches!(k.code, KeyCode::Esc)
            || (ctrl && k.code == KeyCode::Char('e'))
            || (alt && matches!(k.code, KeyCode::Char('b' | 'f')))
        {
            e.clear_selection();
        }
        match k.code {
            KeyCode::Esc => {
                self.session.cancel();
                self.steering = None;
            }
            KeyCode::Enter if shift || alt => e.insert("\n"),
            KeyCode::Char('j') if ctrl => e.insert("\n"),
            KeyCode::Enter => self.submit(),
            KeyCode::Char('c') if primary => {
                if e.is_empty() {
                    self.quit = true;
                } else {
                    e.clear();
                }
            }
            KeyCode::Char('d') if ctrl && e.is_empty() => self.quit = true,
            KeyCode::Char('v') if ctrl || k.modifiers.contains(KeyModifiers::SUPER) => {
                return self.paste_clipboard();
            }
            KeyCode::Char('a') if primary => e.select_all(),
            KeyCode::Char('x') if primary => self.copy_selection(true),
            KeyCode::Char('e') if ctrl => e.end(),
            KeyCode::Char('u') if ctrl => e.kill_to_start(),
            KeyCode::Char('k') if ctrl => e.kill_to_end(),
            KeyCode::Char('w') if ctrl => e.delete_word(),
            KeyCode::Left if ctrl || alt => e.word_left(),
            KeyCode::Right if ctrl || alt => e.word_right(),
            KeyCode::Char('b') if alt => e.word_left(),
            KeyCode::Char('f') if alt => e.word_right(),
            KeyCode::Left => e.left(),
            KeyCode::Right => e.right(),
            KeyCode::Home => e.home(),
            KeyCode::End => e.end(),
            KeyCode::Backspace if alt || ctrl => e.delete_word(),
            KeyCode::Backspace => e.backspace(),
            KeyCode::Delete => e.delete(),
            KeyCode::Up => {
                e.vertical(true, cols);
            }
            KeyCode::Down => {
                e.vertical(false, cols);
            }
            KeyCode::Tab => {}
            KeyCode::Char(c) => e.insert(&c.to_string()),
            _ => {}
        }
        self.finish_edit(before);
    }

    fn copy_selection(&mut self, cut: bool) {
        let text = if !cut && !self.selection.text.is_empty() {
            Some(self.selection.text.clone())
        } else {
            self.editor.selected_text()
        };
        let Some(text) = text else {
            return;
        };
        match crate::clipboard::write(&text) {
            Ok(()) => {
                if cut {
                    self.editor.delete_selection();
                } else {
                    self.editor.clear_selection();
                }
                self.selection = Default::default();
                self.flash("Copied.");
            }
            Err(e) => self.error(e),
        }
    }
    fn question_key(&mut self, code: KeyCode, modified: bool, page: usize) -> bool {
        let Some(q) = &mut self.question else {
            return false;
        };
        let step = self.palette.ui.scroll_step as isize;
        match code {
            KeyCode::Esc => {
                let id = self.question.take().unwrap().id;
                self.restore_draft();
                let _ = self.session.ui_reply(id, None);
            }
            KeyCode::Enter if !modified => {
                let id = self.question.take().unwrap().id;
                let answer = self.editor.text();
                self.restore_draft();
                let _ = self.session.ui_reply(id, Some(answer));
            }
            KeyCode::PageUp => q.panel.scroll_by(-(page as isize)),
            KeyCode::PageDown => q.panel.scroll_by(page as isize),
            KeyCode::Up if self.editor.is_empty() => q.panel.scroll_by(-step),
            KeyCode::Down if self.editor.is_empty() => q.panel.scroll_by(step),
            _ => return false,
        }
        true
    }

    pub(super) fn finish_edit(&mut self, before: Option<Vec<char>>) {
        // The editor is a search box or answer input: queued messages stay queued and
        // `/`, `@`, `#` are plain text.
        if self.searching() {
            let q = self.editor.text();
            if let Some(p) = &mut self.picker {
                p.filter(&q);
            }
            return;
        }
        if self.answering() {
            return;
        }
        if before.is_some_and(|buf| buf != self.editor.buf)
            && let Some(id) = self.steering.take()
        {
            self.session.withdraw_input(id);
        }
        self.update_token_picker();
    }

    pub fn paste(&mut self, s: &str) {
        self.selection = Default::default();
        if self.input_disabled() {
            return;
        }
        if s.is_empty() {
            return self.paste_clipboard();
        }
        let before = self.steering.map(|_| self.editor.buf.clone());
        if self.searching() {
            self.editor.insert(&s.replace('\n', " "));
        } else {
            self.editor.insert(s);
        }
        self.finish_edit(before);
    }
}

#[cfg(test)]
#[path = "keys_tests.rs"]
mod tests;
