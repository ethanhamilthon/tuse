use crate::app::App;
use crate::editor::AtomKind;
use crate::panel::Panel;
use crate::picker::{Kind, Picker, item};
use crate::style::Palette;
use tuse_config::EFFORTS;

pub const COMMANDS: [(&str, &str); 9] = [
    ("config", "show the resolved config"),
    ("new", "start a new session"),
    ("resume", "open a session from this folder"),
    ("models", "switch model and effort"),
    ("tasks", "background tasks"),
    ("usage", "context, tokens and cost"),
    ("trace", "everything in this session"),
    ("reload", "run the config file again"),
    ("quit", "exit"),
];

impl App {
    pub fn run_command(&mut self, name: &str) {
        match name {
            "config" => {
                self.panel = Some(Panel::new("Config", self.session.config().to_json()));
            }
            "new" => self.switch(None),
            "quit" => self.quit = true,
            "resume" => self.open_resume(),
            "models" => {
                self.open_models();
                self.session.fetch_models();
            }
            "tasks" => self.open_tasks(),
            "usage" => {
                self.panel = Some(crate::usage::panel(&self.session, &self.palette));
            }
            "trace" => self.open_trace(),
            "reload" => match self.session.reload() {
                Ok(()) => {
                    self.palette = Palette::from_config(&self.session.config());
                    self.palette_gen += 1;
                    let config = self.session.config();
                    self.chat = self.session.with_records(|r| {
                        crate::chat::Chat::from_records(r, &|m| config.display_name(m))
                    });
                    if let Some(panel) = &mut self.panel {
                        panel.invalidate();
                    }
                    for p in self.session.config().problems() {
                        self.notice(p);
                    }
                    self.flash("Config reloaded.");
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
            self.flash("No other sessions in this folder.");
            return;
        }
        self.open_modal(Picker::new(Kind::Resume, "Resume", items));
    }

    fn open_trace(&mut self) {
        let mut text = String::new();
        for item in self.session.trace() {
            use std::fmt::Write;
            let _ = writeln!(text, "--- {} ---\n{}\n", item.role, item.text);
        }
        let mut panel = Panel::new("Trace", text);
        panel.scroll = usize::MAX;
        self.panel = Some(panel);
    }

    pub fn open_tasks(&mut self) {
        let items: Vec<_> = self
            .session
            .tasks
            .list()
            .into_iter()
            .map(|t| item(format!("{} {}", t.id, t.name), t.status, t.id))
            .collect();
        if items.is_empty() {
            self.close_modal();
            self.flash("No tasks. The agent starts them with `spawn()`.");
            return;
        }
        let selected = self
            .picker
            .as_ref()
            .and_then(|p| p.current())
            .map(|i| i.value.clone());
        let p = Picker::new(Kind::Task, "Tasks (enter: output, ctrl+x: stop)", items);
        let p = self.open_modal(p);
        if let Some(v) = selected {
            p.select(&v);
        }
    }

    /// Enter on a picker item.
    pub fn choose(&mut self) {
        let Some(p) = self.picker.take() else { return };
        // Effort follows Model and keeps the draft aside; everything else gives it back.
        if !(p.kind == Kind::Model && p.current().is_some()) {
            self.restore_draft();
        }
        let value = p.current().map(|i| i.value.clone());
        match (&p.kind, value) {
            (Kind::Command, Some(v)) => {
                self.cut_token();
                self.run_command(&v);
            }
            (Kind::Prompt, Some(v)) => {
                self.cut_token();
                self.editor
                    .insert_atom(&format!("#{v}"), AtomKind::Prompt(v.clone()));
                self.editor.insert(" ");
            }
            (Kind::File, Some(v)) if v.ends_with('/') => self.replace_token(&format!("@{v}")),
            (Kind::File, Some(v)) => {
                self.cut_token();
                self.editor
                    .insert_atom(&format!("@{v}"), AtomKind::File(crate::files::path(&v)));
                self.editor.insert(" ");
            }
            (Kind::Resume, Some(v)) => self.switch(Some(v)),
            (Kind::Model, Some(v)) if !v.is_empty() && !self.session.model_available(&v) => {
                self.restore_draft();
                self.error(format!("Model `{v}` is unavailable"));
            }
            (Kind::Model, Some(v)) => {
                let (_, effort) = self.session.model();
                let items = EFFORTS.iter().map(|e| item(*e, "", *e)).collect();
                let ep = Picker::new(Kind::Effort(v.clone()), format!("Effort for {v}"), items);
                self.open_modal(ep).select(&effort);
            }
            (Kind::Effort(model), Some(v)) => {
                self.session.set_model(model, &v);
                let saved = self
                    .session
                    .save_choice("model", model)
                    .and_then(|()| self.session.save_choice("effort", &v));
                if let Err(e) = saved {
                    self.error(format!("Can't save the model: {e}"));
                }
            }
            (Kind::Task, Some(v)) => {
                if let Some(t) = self.session.tasks.get(&v) {
                    let out = tuse_core::tail(&t.log, 200);
                    let title = format!("Task {} {} [{}]", t.id, t.name, t.status);
                    self.panel = Some(Panel::new(title, out));
                }
            }
            _ => {}
        }
        if self.picker.is_none() {
            self.update_token_picker();
        }
    }
}
