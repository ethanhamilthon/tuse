use crate::kit::clipboard::Pasted;
use crate::ui::app::App;
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
        } else if self.secret_input() {
            self.editor.insert(&s.replace(['\r', '\n'], ""));
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
        match crate::kit::clipboard::read(&dir) {
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
        if let Some(rest) = text.strip_prefix('/') {
            let (name, after) = rest.split_once(char::is_whitespace).unwrap_or((rest, ""));
            let has_ws = rest.len() > name.len();
            if !name.is_empty() && !name.contains('/') {
                if matches!(name, "providers" | "plugins") {
                    match tuse_config::command_words(after) {
                        Ok(words) => {
                            let saved = std::mem::take(&mut self.editor);
                            let ok = if name == "providers" {
                                self.run_providers(&words)
                            } else {
                                self.run_plugins(&words)
                            };
                            if !ok {
                                self.editor = saved;
                            }
                        }
                        Err(e) => self.error(format!("/{name}: {e}")),
                    }
                    return;
                }
                if crate::ui::picker::COMMANDS.iter().any(|(n, _)| *n == name) {
                    if has_ws {
                        self.error(format!("/{name} takes no words"));
                        return;
                    }
                    self.editor.clear();
                    self.run_command(name);
                    return;
                }
                if self.session.config().commands.contains(name) {
                    if has_ws {
                        self.error(format!("/{name} takes no words"));
                    } else if self.session.run_command(name, &[]).is_ok() {
                        self.editor.clear();
                    }
                    return;
                }
                self.error(format!("Unknown command /{name}"));
                return;
            }
        }
        self.picker = None;
        self.scroll = 0;
        let refs = match self.try_refs() {
            Ok(r) => r,
            Err(e) => return self.error(e),
        };
        match tuse_core::build_input(trimmed, refs) {
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

    #[cfg(test)]
    pub(crate) fn refs(&self) -> Refs {
        self.try_refs().unwrap_or_default()
    }

    fn try_refs(&self) -> Result<Refs, String> {
        let refs = self.editor.refs();
        Ok(Refs {
            prompts: self.session.resolve_prompts(&refs.prompts, false)?,
            images: refs.images,
            files: refs.files,
            strict: false,
            cwd: self.cwd.clone(),
        })
    }
}
