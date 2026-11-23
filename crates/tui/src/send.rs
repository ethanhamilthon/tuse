use crate::app::App;
use crate::clipboard::Pasted;
use tuse_core::Refs;

impl App {
    /// Ctrl+V / Cmd+V: an image becomes an `[Image #n]` atom, text is inserted.
    pub fn paste_clipboard(&mut self) {
        if self.input_disabled() {
            return;
        }
        let dir = tuse_core::tuse_home().join(&self.session.id).join("images");
        match crate::clipboard::read(&dir) {
            // A search box or question answer takes no images.
            Ok(Pasted::Image(_)) if self.searching() || self.answering() => {}
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
        if self.steering.is_some() {
            return;
        }
        let text = self.editor.text();
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return;
        }
        if let Some(cmd) = trimmed.strip_prefix('/')
            && let name = cmd.split(' ').next().unwrap_or("")
            && name == cmd
            && (crate::commands::COMMANDS.iter().any(|(n, _)| *n == name)
                || self.session.config().commands.iter().any(|n| n == name))
        {
            self.editor.clear();
            self.run_command(cmd);
            return;
        }
        self.picker = None;
        self.scroll = 0;
        match tuse_core::build_input(trimmed, self.refs()) {
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

    /// The prompts, files and images picked into the input.
    pub(crate) fn refs(&self) -> Refs {
        Refs {
            prompts: self
                .session
                .resolve_prompts(&self.editor.prompts(), false)
                .unwrap_or_default(),
            images: self.editor.images(),
            files: self.editor.files(),
            strict: false,
            cwd: self.cwd.clone(),
        }
    }
}
