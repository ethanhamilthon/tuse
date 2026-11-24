use crate::app::App;
use crate::editor::AtomKind;
use crate::panel::Panel;
use crate::picker::Item;
use crate::picker::{Kind, Picker, item};
use crate::style::Palette;
use tuse_config::EFFORTS;
use tuse_config::ModelDef;

impl App {
    pub(crate) fn restyle(&mut self) {
        self.palette = Palette::for_session(&self.session.config(), &self.cwd);
        self.palette_gen += 1;
        self.chat.invalidate();
        if let Some(panel) = &mut self.panel {
            let scroll = panel.scroll;
            match panel.kind {
                crate::panel::PanelKind::Config => {
                    *panel = crate::config_view::panel(&self.session.config(), &self.palette);
                    panel.scroll = scroll;
                }
                crate::panel::PanelKind::Session => {
                    *panel = crate::session_view::panel(&self.session, &self.palette);
                    panel.scroll = scroll;
                }
                crate::panel::PanelKind::Plain => panel.invalidate(),
            }
        }
        if let Some(q) = &mut self.question {
            q.panel.invalidate();
        }
    }
    pub fn run_command(&mut self, name: &str) {
        match name {
            "config" => {
                self.trace_back = None;
                self.trace_items.clear();
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
            "session" => {
                self.trace_back = None;
                self.trace_items.clear();
                self.panel = Some(crate::session_view::panel(&self.session, &self.palette));
            }
            "trace" => self.open_trace(),
            "reload" => match self.session.reload() {
                Ok(()) => {
                    self.restyle();
                    self.chat = crate::chat::Chat::from_session(&self.session);
                    self.streaming = None;
                    self.thinking = None;
                    self.pending_tools.clear();
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
        let items: Vec<_> = tuse_core::Meta::list(&self.cwd)
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
            .tasks()
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

    pub fn choose(&mut self) {
        let Some(p) = self.picker.take() else { return };
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
            (Kind::File, Some(v)) => self.insert_file(&v),
            (Kind::Resume, Some(v)) => self.switch(Some(v)),
            (Kind::Model, Some(v)) if !v.is_empty() && !self.session.model_available(&v) => {
                self.restore_draft();
                self.error(format!("Model `{v}` is unavailable"));
            }
            (Kind::Model, Some(v)) => {
                let (_, effort) = self.session.model();
                let cfg = self.session.config();
                let provider = tuse_config::split_model(&v).0;
                let supported = cfg
                    .connect(provider)
                    .map(|c| tuse_core::supported_efforts(c.api_type()))
                    .unwrap_or(&EFFORTS);
                let items = EFFORTS
                    .iter()
                    .map(|e| {
                        item(
                            *e,
                            if supported.contains(e) {
                                ""
                            } else {
                                "unsupported"
                            },
                            *e,
                        )
                    })
                    .collect();
                let ep = Picker::new(Kind::Effort(v.clone()), items);
                self.open_modal(ep).select(&effort);
            }
            (Kind::Effort(model), Some(v)) => {
                if let Err(e) = self.session.select_model(model, &v, true) {
                    self.error(format!("Can't save the model: {e}"));
                }
            }
            (Kind::Task, Some(v)) => {
                if let Some(t) = self.session.task(&v) {
                    let out = tuse_core::Tasks::tail(&t.log, 200);
                    self.trace_back = None;
                    self.trace_items.clear();
                    self.panel = Some(Panel::new(out));
                }
            }
            (Kind::Trace, Some(v)) => {
                if let Ok(idx) = v.parse::<usize>()
                    && let Some(it) = self.trace_items.get(idx)
                {
                    let panel = crate::panel::trace_panel(it);
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

impl App {
    pub fn pick_file(&mut self) {
        let Some(p) = self.picker.take() else { return };
        match p.current().map(|i| i.value.clone()) {
            Some(v) => self.insert_file(&v),
            None => self.picker = Some(p),
        }
    }

    fn insert_file(&mut self, v: &str) {
        self.cut_token();
        self.editor
            .insert_atom(&format!("@{v}"), AtomKind::File(crate::files::path(v)));
        self.editor.insert(" ");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_detail_repeats_the_name_only_under_a_display_name() {
        let opus = ModelDef {
            name: "c/opus".into(),
            window: 200_000,
            ..Default::default()
        };
        let sonnet = ModelDef {
            name: "c/sonnet".into(),
            display: "Sonnet".into(),
            window: 200_000,
            ..Default::default()
        };
        let plain = model_item(Some(&opus), "c/opus", true);
        assert_eq!(
            (plain.label.as_str(), plain.detail.as_str()),
            ("c/opus", "200k ctx")
        );
        let named = model_item(Some(&sonnet), "c/sonnet", true);
        assert_eq!(named.detail, "c/sonnet · 200k ctx");
        assert_eq!(model_item(None, "c/other", true).detail, "");
        assert_eq!(
            model_item(Some(&opus), "c/opus", false).detail,
            "unavailable · 200k ctx"
        );
    }

    #[test]
    fn docs_and_config_list_every_built_in_command() {
        let docs = tuse_core::docs(Some("commands"));
        for (name, _) in crate::picker::COMMANDS {
            assert!(docs.contains(&format!("`/{name}`")), "docs misses /{name}");
            assert!(tuse_config::BUILTIN_COMMANDS.contains(&name), "/{name}");
        }
        assert_eq!(
            crate::picker::COMMANDS.len(),
            tuse_config::BUILTIN_COMMANDS.len()
        );
    }
}

impl App {
    pub fn open_models(&mut self) {
        let config = self.session.config();
        let (current, _) = self.session.model();
        let mut names = self.session.available_models();
        if !names.contains(&current) {
            names.insert(0, current.clone());
        }
        names.sort();
        let items: Vec<Item> = names
            .iter()
            .map(|n| {
                let def = config.model_def(n);
                model_item(def, n, self.session.model_available(n))
            })
            .collect();
        let selected = self
            .picker
            .as_ref()
            .filter(|p| p.kind == Kind::Model)
            .and_then(|p| p.current())
            .map_or(current, |i| i.value.clone());
        self.open_modal(Picker::new(Kind::Model, items))
            .select(&selected);
    }
}

fn model_item(def: Option<&ModelDef>, name: &str, available: bool) -> Item {
    let shown = tuse_config::display_name(def, name);
    let mut parts = if shown == name {
        vec![]
    } else {
        vec![name.to_string()]
    };
    if !available && !name.is_empty() {
        parts.push("unavailable".to_string());
    }
    if let Some(m) = def {
        parts.extend(details(m));
    }
    item(shown, parts.join(" · "), name)
}

fn details(m: &ModelDef) -> Vec<String> {
    let mut out = vec![];
    if m.window > 0 {
        out.push(format!("{} ctx", tuse_core::tokens(m.window)));
    }
    if let Some(p) = &m.prices {
        out.push(format!("${}/${} per 1M", p.input, p.output));
    }
    out
}
