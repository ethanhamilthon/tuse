use crate::app::App;
use crate::clipboard::Pasted;
use crate::expand_prompts::expand_prompts;

impl App {
    /// Ctrl+V / Cmd+V: an image becomes an `[Image #n]` atom, text is inserted.
    pub fn paste_clipboard(&mut self) {
        let dir = tuse_core::tuse_home().join(&self.session.id).join("images");
        match crate::clipboard::read(&dir) {
            Ok(Pasted::Image(path)) => {
                let before = self.steering.map(|_| self.editor.buf.clone());
                self.editor.insert_image(path);
                self.editor.insert(" ");
                self.finish_edit(before);
            }
            Ok(Pasted::Text(t)) => self.paste(&t),
            Ok(Pasted::Nothing) => {}
            Err(e) => self.error(format!("Clipboard: {e}")),
        }
    }

    pub(super) fn submit(&mut self) {
        if self.ask_input.is_some() {
            let answer = self.editor.text_expanded().trim().to_string();
            if !answer.is_empty() {
                self.finish_answer(Some(answer));
            }
            return;
        }
        if self.steering.is_some() {
            return;
        }
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
        let expanded = expand_prompts(
            self.editor.text_expanded().trim(),
            &self.session.config().prompts,
        );
        let images = self.editor.images();
        self.picker = None;
        self.scroll = 0;
        let images: Vec<String> = images.iter().map(|p| p.display().to_string()).collect();
        match tuse_core::with_images(&expanded, &images) {
            Ok(input) => {
                let (id, queued) = self.session.submit(input);
                if queued {
                    self.steering = Some(id);
                } else {
                    self.editor.clear();
                }
            }
            Err(e) => self.error(e),
        }
    }
}
