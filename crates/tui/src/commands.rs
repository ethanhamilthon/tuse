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
                self.panel = Some(crate::config_view::panel(
                    &self.session.config(),
                    &self.palette,
                ));
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
                    self.chat = crate::chat::Chat::from_session(&self.session);
                    if let Some(panel) = &mut self.panel {
                        panel.invalidate();
                    }
                    if let Some(q) = &mut self.question {
                        q.panel.invalidate();
                    }
                    for p in self.session.config().problems() {
                        self.notice(p);
                    }
                    self.flash("Config reloaded.");
                }
                Err(e) => self.error(format!("Reload failed: {e}")),
            },
            other if self.session.config().commands.iter().any(|c| c == other) => {
                if let Err(e) = self.session.run_command(other) {
                    self.error(e);
                }
            }
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
        self.open_modal(Picker::new(Kind::Resume, items));
    }

    fn open_trace(&mut self) {
        let items = self.session.trace();
        let rows: Vec<_> = items
            .iter()
            .enumerate()
            .map(|(i, it)| {
                item(
                    format!("{i}  {}  {}", it.role, it.summary),
                    "",
                    i.to_string(),
                )
            })
            .collect();
        self.trace_items = items;
        self.trace_back = None;
        self.open_modal(Picker::new(Kind::Trace, rows));
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
        let p = Picker::new(Kind::Task, items);
        let p = self.open_modal(p);
        if let Some(v) = selected {
            p.select(&v);
        }
    }

    /// Enter on a picker item.
    pub fn choose(&mut self) {
        let Some(p) = self.picker.take() else { return };
        // Effort follows Model and Trace detail returns to Trace list; both keep the draft aside.
        let keep_draft = matches!(p.kind, Kind::Model | Kind::Trace) && p.current().is_some();
        if !keep_draft {
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
                let ep = Picker::new(Kind::Effort(v.clone()), items);
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
                    self.panel = Some(Panel::new(out));
                }
            }
            (Kind::Trace, Some(v)) => {
                if let Ok(idx) = v.parse::<usize>()
                    && let Some(it) = self.trace_items.get(idx)
                {
                    let panel = match it.role {
                        "system" | "user" | "agent" => Panel::markdown(it.text.clone()),
                        "lua" => Panel::lua(it.text.clone()),
                        _ => Panel::new(it.text.clone()),
                    };
                    let search = std::mem::take(&mut self.editor);
                    self.trace_back = Some((p, search));
                    self.panel = Some(panel);
                } else {
                    self.restore_draft();
                }
            }
            (Kind::LuaPicker(id), v) => {
                let _ = self.session.ui_reply(*id, v);
            }
            _ => {}
        }
        if self.picker.is_none() {
            self.update_token_picker();
        }
    }
}
