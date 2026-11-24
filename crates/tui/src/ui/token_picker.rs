use crate::ui::app::App;
use crate::ui::picker::COMMANDS;
use crate::ui::picker::{Kind, Picker, item};

impl App {
    pub(crate) fn cut_token(&mut self) {
        self.replace_token("");
    }

    pub(crate) fn replace_token(&mut self, with: &str) {
        self.editor.replace_token(with);
    }

    pub fn update_token_picker(&mut self) {
        if self.steering.is_some()
            || self.answering()
            || self.picker.as_ref().is_some_and(|p| !p.kind.is_token())
        {
            return;
        }
        let (_, token) = self.editor.token();
        if self.dismissed.as_deref() == Some(&token) {
            return;
        }
        self.dismissed = None;
        let mut chars = token.chars();
        let (kind, query) = match chars.next() {
            Some('/') => (Kind::Command, chars.as_str()),
            Some('#') if !self.session.config().prompts.is_empty() => {
                (Kind::Prompt, chars.as_str())
            }
            Some('@') => (Kind::File, chars.as_str()),
            _ => {
                self.picker = None;
                return;
            }
        };
        let mut query = query.to_string();
        let mut dir = String::new();
        if kind == Kind::File {
            let (d, f) = crate::kit::files::split(&query);
            (dir, query) = (d.to_string(), f.to_string());
        }
        let stale = kind == Kind::File && self.file_dir != dir;
        if stale || self.picker.as_ref().is_none_or(|p| p.kind != kind) {
            self.file_dir = dir;
            let items = self.token_items(&kind);
            self.picker = Some(Picker::new(kind, items));
        }
        if let Some(p) = &mut self.picker {
            p.filter(&query);
        }
    }

    fn token_items(&mut self, kind: &Kind) -> Vec<crate::ui::picker::Item> {
        match kind {
            Kind::Command => {
                let mut items: Vec<_> = COMMANDS
                    .iter()
                    .map(|(n, d)| item(format!("/{n}"), *d, *n))
                    .collect();
                for name in &self.session.config().commands {
                    items.push(item(format!("/{name}"), "", name.clone()));
                }
                items
            }
            Kind::Prompt => {
                let config = self.session.config();
                config
                    .prompts
                    .iter()
                    .map(|(k, v)| item(format!("#{k}"), v.lines().next().unwrap_or(""), k.clone()))
                    .collect()
            }
            _ => {
                let max = crate::kit::style::FILE_PICKER_MAX;
                crate::kit::files::list(&self.cwd, &self.file_dir, max)
                    .into_iter()
                    .map(|f| item(f.strip_prefix(&self.file_dir).unwrap_or(&f), "", f.clone()))
                    .collect()
            }
        }
    }
}
