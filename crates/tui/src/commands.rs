use crate::app::App;
use crate::panel::Panel;
use crate::picker::{Kind, Picker, item};
use crate::style::Palette;
use tuse_config::EFFORTS;

pub const COMMANDS: [(&str, &str); 9] = [
    ("new", "start a new session"),
    ("resume", "open a session from this folder"),
    ("models", "switch model and effort"),
    ("theme", "switch the theme"),
    ("tasks", "background tasks"),
    ("usage", "context, tokens and cost"),
    ("trace", "everything in this session"),
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
                self.panel = Some(crate::usage::panel(&self.session, &self.palette));
                self.trace_item = None;
            }
            "trace" => self.open_trace(None),
            "reload" => match self.session.reload() {
                Ok(()) => {
                    self.palette = Palette::from_config(&self.session.config());
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

    /// The whole session as a list; Enter opens an item, Esc in the item comes back here.
    pub fn open_trace(&mut self, select: Option<usize>) {
        let all = self.session.trace();
        let items = all
            .iter()
            .enumerate()
            .map(|(i, it)| {
                crate::picker::tagged(it.role, format!("{i} {}", it.summary), i.to_string())
            })
            .collect();
        self.trace = all.into_iter().map(|it| (it.role, it.text)).collect();
        let mut p = Picker::new(Kind::Trace, "Trace", items);
        let last = self.trace.len().saturating_sub(1);
        p.select(&select.unwrap_or(last).to_string());
        self.picker = Some(p);
        self.panel = None;
        self.trace_item = None;
    }

    pub fn open_trace_item(&mut self, at: usize) {
        if let Some((role, text)) = self.trace.get(at) {
            let title = format!("Trace {}/{} {role}", at + 1, self.trace.len());
            self.panel = Some(Panel::new(title, text.clone()));
            self.trace_item = Some(at);
        }
    }

    pub fn open_tasks(&mut self) {
        let items: Vec<_> = self
            .session
            .tasks
            .list()
            .into_iter()
            .map(|t| {
                let status = match (t.status.as_str(), t.exit) {
                    ("exited", Some(c)) => format!("exited {c}"),
                    ("idle" | "running", _) if t.queue > 0 => {
                        format!("{}, {} queued", t.status, t.queue)
                    }
                    (s, _) => s.to_string(),
                };
                item(format!("{} {}", t.id, t.name), status, t.id)
            })
            .collect();
        if items.is_empty() {
            self.picker = None;
            self.notice("No tasks. The agent starts them with `task_run()`.");
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
            }
            (Kind::Task, Some(v)) => {
                if let Some(t) = self.session.tasks.get(&v) {
                    let out = tuse_core::tail(&t.log, 200);
                    let title = format!("Task {} {} [{}]", t.id, t.name, t.status);
                    self.panel = Some(Panel::new(title, format!("{}\n\n{out}", t.code)));
                }
            }
            (Kind::Trace, Some(v)) => {
                let at: usize = v.parse().unwrap_or(0);
                self.open_trace_item(at);
            }
            (Kind::Ask(id), v) => {
                // Typed text is the answer; without it the selected option (the last row is only a hint).
                let typed = p.query.trim();
                let custom = (!typed.is_empty()).then(|| typed.to_string());
                let Some(answer) = custom.or(v.filter(|v| !v.is_empty())) else {
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
}
