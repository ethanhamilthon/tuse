use crate::app::App;
use crate::picker::{Kind, Picker, item};
use crate::style::Palette;
use tuse_config::EFFORTS;

pub const COMMANDS: [(&str, &str); 8] = [
    ("new", "start a new session"),
    ("resume", "open a session from this folder"),
    ("models", "switch model and effort"),
    ("theme", "switch the theme"),
    ("tasks", "background tasks"),
    ("usage", "context, tokens and cost"),
    ("reload", "rebuild the config from its files"),
    ("quit", "exit"),
];

impl App {
    pub fn run_command(&mut self, name: &str) {
        match name {
            "new" => self.switch(None),
            "quit" => self.quit = true,
            "resume" => self.open_resume(),
            "models" => self.open_models(),
            "theme" => self.open_themes(),
            "tasks" => self.open_tasks(),
            "usage" => {
                let r = self.session.usage_report();
                self.notice(r);
            }
            "reload" => match self.session.reload() {
                Ok(()) => {
                    self.palette = Palette::new(&self.session.config().active_theme());
                    self.chat = crate::chat::Chat::from_records(&self.session.records());
                    self.notice("Config reloaded.");
                }
                Err(e) => self.error(format!("Reload failed: {e}")),
            },
            other => self.error(format!("Unknown command /{other}")),
        }
    }

    fn open_resume(&mut self) {
        let items: Vec<_> = tuse_core::list_sessions(&self.cwd)
            .into_iter()
            .filter(|m| m.id != self.session.id)
            .map(|m| item(m.title, m.updated.replace('T', " ").replace('Z', ""), m.id))
            .collect();
        if items.is_empty() {
            self.notice("No other sessions in this folder.");
            return;
        }
        self.picker = Some(Picker::new(Kind::Resume, "Resume", items));
    }

    pub fn open_tasks(&mut self) {
        let items: Vec<_> = self
            .session
            .tasks
            .list()
            .into_iter()
            .map(|t| {
                let status = match (t.status.as_str(), t.code) {
                    ("exited", Some(c)) => format!("exited {c}"),
                    (s, _) => s.to_string(),
                };
                item(format!("{} {}", t.id, t.name), status, t.id)
            })
            .collect();
        if items.is_empty() {
            self.picker = None;
            self.notice("No tasks. The agent starts them with `tuse task run`.");
            return;
        }
        let selected = self
            .picker
            .as_ref()
            .and_then(|p| p.current())
            .map(|i| i.value.clone());
        let mut p = Picker::new(Kind::Task, "Tasks (enter: output, ctrl+x: stop)", items);
        if let Some(v) = selected {
            p.select(&v);
        }
        self.picker = Some(p);
    }

    /// Enter on a picker item.
    pub fn choose(&mut self) {
        let Some(p) = self.picker.take() else { return };
        let value = p.current().map(|i| i.value.clone());
        match (&p.kind, value) {
            (Kind::Command, Some(v)) => {
                self.editor.clear();
                self.run_command(&v);
            }
            (Kind::Prompt, Some(v)) => {
                self.cut_token();
                self.editor.insert_atom(&format!("#{v}"), None);
                self.editor.insert(" ");
            }
            (Kind::File, Some(v)) if v.ends_with('/') => self.replace_token(&format!("@{v}*")),
            (Kind::File, Some(v)) => {
                self.cut_token();
                self.editor.insert_atom(&format!("@{v}"), None);
                self.editor.insert(" ");
            }
            (Kind::Resume, Some(v)) => self.switch(Some(v)),
            (Kind::Theme, Some(v)) => self.set_theme(&v),
            (Kind::Model, Some(v)) => {
                let (_, effort) = self.session.model();
                let items = EFFORTS.iter().map(|e| item(*e, "", *e)).collect();
                let mut ep = Picker::new(Kind::Effort(v.clone()), format!("Effort for {v}"), items);
                ep.select(&effort);
                self.picker = Some(ep);
            }
            (Kind::Effort(model), Some(v)) => {
                self.session.set_model(model, &v);
                let saved = self
                    .session
                    .save_setting("model", model)
                    .and_then(|()| self.session.save_setting("effort", &v));
                if let Err(e) = saved {
                    self.error(format!("Can't save the model: {e}"));
                }
                let name = self.session.config().display_name(model);
                self.notice(format!("Model {name}, effort {v}."));
            }
            (Kind::Task, Some(v)) => {
                if let Some(t) = self.session.tasks.get(&v) {
                    let out = tuse_core::tail(&t.log, 20);
                    self.notice(format!(
                        "{} {} [{}]\n$ {}\n{out}",
                        t.id, t.name, t.status, t.command
                    ));
                }
            }
            (Kind::Ask(id), v) => {
                let custom = (!p.query.trim().is_empty()).then(|| p.query.clone());
                let Some(answer) = v.or(custom) else {
                    self.picker = Some(p);
                    return;
                };
                let id = id.clone();
                self.session.answer(&id, Some(answer));
                self.asks.retain(|a| a.0 != id);
                self.next_ask();
            }
            _ => {}
        }
        if self.picker.is_none() {
            self.update_token_picker();
        }
    }

    fn cut_token(&mut self) {
        self.replace_token("");
    }

    fn replace_token(&mut self, with: &str) {
        let (start, _) = self.editor.token();
        let end = self.editor.cursor;
        self.editor.replace(start, end, with);
    }

    /// Opens, filters or closes the `/`, `#` and `@` pickers for the token at the cursor.
    pub fn update_token_picker(&mut self) {
        if self.picker.as_ref().is_some_and(|p| !p.kind.is_token()) {
            return;
        }
        let (start, token) = self.editor.token();
        if self.dismissed.as_deref() == Some(&token) {
            return;
        }
        self.dismissed = None;
        let mut chars = token.chars();
        let (kind, query) = match chars.next() {
            Some('/') if start == 0 => (Kind::Command, chars.as_str()),
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
            let (d, f) = crate::files::split(&query);
            (dir, query) = (d.to_string(), f.to_string());
        }
        let stale = kind == Kind::File && self.file_dir != dir;
        if stale || self.picker.as_ref().is_none_or(|p| p.kind != kind) {
            self.file_dir = dir;
            let items = self.token_items(&kind);
            self.picker = Some(Picker::new(kind, "", items));
        }
        if let Some(p) = &mut self.picker {
            p.filter(&query);
        }
    }

    fn token_items(&mut self, kind: &Kind) -> Vec<crate::picker::Item> {
        match kind {
            Kind::Command => COMMANDS
                .iter()
                .map(|(n, d)| item(format!("/{n}"), *d, *n))
                .collect(),
            Kind::Prompt => {
                let config = self.session.config();
                config
                    .prompts
                    .iter()
                    .map(|(k, v)| item(format!("#{k}"), v.lines().next().unwrap_or(""), k.clone()))
                    .collect()
            }
            _ => {
                let max = self.palette.t.file_picker_max;
                crate::files::list(&self.cwd, &self.file_dir, max)
                    .into_iter()
                    .map(|f| item(f.clone(), "", f))
                    .collect()
            }
        }
    }
}
