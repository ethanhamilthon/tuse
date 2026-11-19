use crate::app::App;
use crate::clipboard::Pasted;
use crate::picker::Kind;
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use std::collections::BTreeMap;

impl App {
    pub fn picker_rows(&self) -> usize {
        let max = self.palette.t.picker_max_height as usize;
        match &self.picker {
            Some(p) if !p.title.is_empty() => max.saturating_sub(1).max(1),
            _ => max.max(1),
        }
    }

    pub fn key(&mut self, k: KeyEvent, page: usize) {
        if k.kind == KeyEventKind::Release {
            return;
        }
        let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
        let alt = k.modifiers.contains(KeyModifiers::ALT);
        let shift = k.modifiers.contains(KeyModifiers::SHIFT);
        if self.picker.is_some() && self.picker_key(k, ctrl, shift) {
            return;
        }
        let e = &mut self.editor;
        match k.code {
            KeyCode::Esc => {
                if self.session.busy() {
                    self.session.cancel();
                }
            }
            KeyCode::Enter if shift || alt => e.insert("\n"),
            KeyCode::Char('j') if ctrl => e.insert("\n"),
            KeyCode::Enter => self.submit(),
            KeyCode::Char('c') if ctrl => {
                if e.is_empty() {
                    self.quit = true;
                } else {
                    e.clear();
                }
            }
            KeyCode::Char('d') if ctrl && e.is_empty() => self.quit = true,
            KeyCode::Char('v') if ctrl || k.modifiers.contains(KeyModifiers::SUPER) => {
                self.paste_clipboard()
            }
            KeyCode::Char('a') if ctrl => e.home(),
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
                if !(e.multiline() && e.vertical(true)) {
                    self.scroll += self.palette.t.scroll_step as usize;
                }
            }
            KeyCode::Down => {
                if !(e.multiline() && e.vertical(false)) {
                    self.scroll = self
                        .scroll
                        .saturating_sub(self.palette.t.scroll_step as usize);
                }
            }
            KeyCode::PageUp => self.scroll += page,
            KeyCode::PageDown => self.scroll = self.scroll.saturating_sub(page),
            KeyCode::Tab => {}
            KeyCode::Char(c) => e.insert(&c.to_string()),
            _ => {}
        }
        self.update_token_picker();
    }

    /// Returns true when the picker consumed the key.
    fn picker_key(&mut self, k: KeyEvent, ctrl: bool, shift: bool) -> bool {
        let rows = self.picker_rows();
        let Some(p) = &mut self.picker else {
            return false;
        };
        match k.code {
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

    pub fn paste(&mut self, s: &str) {
        if let Some(p) = &mut self.picker
            && !p.kind.is_token()
        {
            let q = format!("{}{}", p.query, s.replace('\n', " "));
            p.filter(&q);
            return;
        }
        if s.is_empty() {
            return self.paste_clipboard();
        }
        self.editor.insert(s);
        self.update_token_picker();
    }

    /// Ctrl+V / Cmd+V: an image becomes an `[Image #n: PATH]` atom, text is inserted.
    pub fn paste_clipboard(&mut self) {
        let dir = tuse_core::tuse_home().join(&self.session.id).join("images");
        match crate::clipboard::read(&dir) {
            Ok(Pasted::Image(path)) => {
                let n = self.editor.images().len() + 1;
                let label = format!("[Image #{n}: {}]", path.display());
                self.editor.insert_atom(&label, Some(path));
                self.editor.insert(" ");
            }
            Ok(Pasted::Text(t)) => self.paste(&t),
            Ok(Pasted::Nothing) => {}
            Err(e) => self.error(format!("Clipboard: {e}")),
        }
    }

    fn submit(&mut self) {
        let text = self.editor.text();
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return;
        }
        if let Some(cmd) = trimmed.strip_prefix('/')
            && crate::commands::COMMANDS.iter().any(|(n, _)| *n == cmd)
        {
            self.editor.clear();
            self.run_command(cmd);
            return;
        }
        let expanded = expand_prompts(trimmed, &self.session.config().prompts);
        let images = self.editor.images();
        self.editor.clear();
        self.picker = None;
        self.scroll = 0;
        let images: Vec<String> = images.iter().map(|p| p.display().to_string()).collect();
        match tuse_core::with_images(&expanded, &images) {
            Ok(input) => self.session.submit(input),
            Err(e) => self.error(e),
        }
    }
}

/// Replaces every `#name` token with the text of prompt `name`.
pub fn expand_prompts(text: &str, prompts: &BTreeMap<String, String>) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(i) = rest.find('#') {
        let at_start = i == 0 || rest[..i].ends_with(char::is_whitespace);
        let name: String = rest[i + 1..]
            .chars()
            .take_while(|c| c.is_alphanumeric() || matches!(c, '-' | '_' | '.'))
            .collect();
        let name = name.trim_end_matches('.');
        out.push_str(&rest[..i]);
        match prompts.get(name) {
            Some(p) if at_start && !name.is_empty() => {
                out.push_str(p.trim_end());
                rest = &rest[i + 1 + name.len()..];
            }
            _ => {
                out.push('#');
                rest = &rest[i + 1..];
            }
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn expands_prompt_tokens() {
        let mut p = std::collections::BTreeMap::new();
        p.insert("non-stop".to_string(), "Do it all.".to_string());
        assert_eq!(super::expand_prompts("fix #non-stop", &p), "fix Do it all.");
        assert_eq!(super::expand_prompts("a#non-stop #x", &p), "a#non-stop #x");
    }
}
