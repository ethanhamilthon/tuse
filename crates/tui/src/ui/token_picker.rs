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
        let arg_of = self.editor.prompt_before_token().filter(|n| {
            self.session
                .config()
                .prompts
                .get(n)
                .is_some_and(|p| !p.args.is_empty())
        });
        let command_arg = match arg_of {
            Some(_) => None,
            None => self
                .editor
                .command_before_token()
                .map(|(_, n)| n)
                .filter(|n| {
                    self.session
                        .config()
                        .commands
                        .get(n)
                        .is_some_and(|c| !c.args.is_empty())
                }),
        };
        let mut chars = token.chars();
        let first = if arg_of.is_some() {
            Some(' ')
        } else {
            chars.next()
        };
        let (kind, query) = match first {
            _ if command_arg.is_some() => (
                Kind::CommandArg(command_arg.unwrap_or_default()),
                token.as_str(),
            ),
            Some(' ') => (Kind::PromptArg(arg_of.unwrap_or_default()), token.as_str()),
            Some('/') if !chars.as_str().contains('/') => (Kind::Command, chars.as_str()),
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
                for (name, c) in &self.session.config().commands {
                    items.push(item(format!("/{name}"), c.desc.as_str(), name.clone()));
                }
                items
            }
            Kind::Prompt => {
                let config = self.session.config();
                config
                    .prompts
                    .iter()
                    .map(|(k, v)| {
                        let detail = if v.args.is_empty() {
                            v.body.lines().next().unwrap_or("").to_string()
                        } else {
                            v.args.join(" | ")
                        };
                        item(format!("#{k}"), detail, k.clone())
                    })
                    .collect()
            }
            Kind::CommandArg(name) => self
                .session
                .config()
                .commands
                .get(name)
                .map(|c| c.args.iter().map(|a| item(a, "", a)).collect())
                .unwrap_or_default(),
            Kind::PromptArg(name) => self
                .session
                .config()
                .prompts
                .get(name)
                .map(|p| p.args.iter().map(|a| item(a, "", a)).collect())
                .unwrap_or_default(),
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
