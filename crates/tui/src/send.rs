use crate::app::App;
use crate::clipboard::Pasted;
use tuse_core::Refs;

impl App {
    pub(super) fn finish_edit(&mut self, before: Option<Vec<char>>) {
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
        if before.is_some_and(|buf| self.editor.differs_from(&buf))
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
        let before = self.steering.map(|_| self.editor.snapshot());
        if self.searching() {
            self.editor.insert(&s.replace('\n', " "));
        } else {
            self.editor.insert(s);
        }
        self.finish_edit(before);
    }

    pub fn paste_clipboard(&mut self) {
        if self.input_disabled() {
            return;
        }
        let dir = tuse_core::tuse_home().join(&self.session.id).join("images");
        match crate::clipboard::read(&dir) {
            Ok(Pasted::Image(_)) if self.searching() || self.answering() => {}
            Ok(Pasted::Image(path)) => {
                let before = self.steering.map(|_| self.editor.snapshot());
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
            && (crate::picker::COMMANDS.iter().any(|(n, _)| *n == name)
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

    pub(crate) fn refs(&self) -> Refs {
        let refs = self.editor.refs();
        Refs {
            prompts: self
                .session
                .resolve_prompts(&refs.prompts, false)
                .unwrap_or_default(),
            images: refs.images,
            files: refs.files,
            strict: false,
            cwd: self.cwd.clone(),
        }
    }
}
