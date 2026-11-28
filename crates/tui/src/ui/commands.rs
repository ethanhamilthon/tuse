use crate::kit::editor::AtomKind;
use crate::kit::style::Palette;
use crate::ui::app::App;
use crate::ui::panel::Panel;
use crate::ui::picker::Item;
use crate::ui::picker::{Kind, Picker, item};
use tuse_config::ModelDef;

impl App {
    pub(crate) fn restyle(&mut self) {
        self.palette = Palette::for_session(&self.session.config(), &self.cwd);
        self.palette_gen += 1;
        self.chat.invalidate();
        if let Some(panel) = &mut self.panel {
            let fresh = match panel.kind {
                crate::ui::panel::PanelKind::Config => Some(crate::ui::screen::config_view::panel(
                    &self.session.config(),
                    &self.palette,
                )),
                crate::ui::panel::PanelKind::Session => Some(
                    crate::ui::screen::session_view::panel(&self.session, &self.palette),
                ),
                crate::ui::panel::PanelKind::Plain => None,
            };
            match fresh {
                Some(fresh) => *panel = fresh.rebuilt(panel),
                None => panel.invalidate(),
            }
        }
        if let Some(q) = &mut self.question {
            q.panel.invalidate();
        }
        if let Some(f) = &mut self.form {
            f.panel.invalidate();
        }
    }
    pub fn run_command(&mut self, name: &str) {
        match name {
            "config" => {
                self.history_back = None;
                self.history_items.clear();
                self.panel = Some(crate::ui::screen::config_view::panel(
                    &self.session.config(),
                    &self.palette,
                ));
            }
            "new" => self.switch(None),
            "quit" => self.quit = true,
            "resume" => self.open_resume(),
            "fork" => self.open_fork(),
            "models" => {
                self.open_models();
                self.session.fetch_models();
            }
            "providers" => {
                self.run_providers(&[]);
            }
            "plugins" => {
                self.run_plugins(&[]);
            }
            "tasks" => self.open_tasks(),
            "session" => {
                self.history_back = None;
                self.history_items.clear();
                self.panel = Some(crate::ui::screen::session_view::panel(
                    &self.session,
                    &self.palette,
                ));
            }
            "history" => self.open_history(),
            "stop" => {
                self.session.cancel();
                self.steering = None;
                self.flash("Stopped.");
            }
            "reload" => {
                if self.session.reload().is_ok() {
                    self.restyle();
                    self.flash("Config reloaded.");
                }
            }
            other if self.session.config().commands.contains(other) => {
                self.run_custom(other, vec![]);
            }
            other => self.error(format!("Unknown command /{other}")),
        }
    }

    pub(crate) fn run_custom(&mut self, name: &str, words: Vec<String>) {
        let _ = self.session.run_command(name, &words);
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

    fn open_fork(&mut self) {
        let items: Vec<_> = self
            .session
            .history()
            .into_iter()
            .enumerate()
            .filter(|(_, it)| matches!(it.role, "user" | "agent") && !it.is_empty_agent())
            .rev()
            .map(|(i, it)| item(it.role, it.summary, i.to_string()))
            .collect();
        if items.is_empty() {
            self.flash("Nothing to fork yet.");
            return;
        }
        self.open_modal(Picker::new(Kind::Fork, items));
    }

    fn open_history(&mut self) {
        let (rows, items) = self
            .session
            .history()
            .into_iter()
            .enumerate()
            .filter(|(_, it)| !it.is_empty_agent() && !matches!(it.role, "meta" | "usage"))
            .enumerate()
            .map(|(visible, (i, it))| {
                let mut row = history_row(i, &it);
                row.value = visible.to_string();
                (row, it)
            })
            .unzip();
        self.history_items = items;
        self.history_back = None;
        self.open_modal(Picker::new(Kind::History, rows));
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
            self.flash("No tasks. The agent starts them with `tasks.spawn()`.");
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
        let keep_draft = matches!(p.kind, Kind::Model | Kind::History) && p.current().is_some();
        if !keep_draft {
            self.restore_draft();
        }
        let value = p.current().map(|i| i.value.clone());
        match (&p.kind, value) {
            (Kind::Command, Some(v)) if matches!(v.as_str(), "providers" | "plugins") => {
                self.replace_token(&format!("/{v} "));
                self.dismissed = None;
                self.update_token_picker();
            }
            (Kind::Command, Some(v)) => {
                self.cut_token();
                self.run_command(&v);
            }
            (Kind::CommandArg(name), Some(v)) if name == "providers" => {
                self.editor.cut_command_arg();
                self.run_providers(&[v]);
            }
            (Kind::CommandArg(name), Some(v)) if name == "plugins" => {
                self.editor.cut_command_arg();
                self.run_plugins(&[v]);
            }
            (Kind::CommandArg(name), None) => {
                let typed = self.editor.token().1;
                let ran = match name.as_str() {
                    "providers" => self.run_providers(&[typed]),
                    "plugins" => self.run_plugins(&[typed]),
                    _ => false,
                };
                if ran {
                    self.editor.cut_command_arg();
                }
            }
            (Kind::Prompt, Some(v)) => {
                self.cut_token();
                self.editor
                    .insert_atom(&format!("#{v}"), AtomKind::Prompt(v.clone()));
                self.editor.insert(" ");
                self.dismissed = None;
                self.update_token_picker();
            }
            (Kind::File, Some(v)) if v.ends_with('/') => self.replace_token(&format!("@{v}")),
            (Kind::File, Some(v)) => self.insert_file(&v),
            (Kind::Resume, Some(v)) => self.switch(Some(v)),
            (Kind::Fork, Some(v)) => self.fork(v.parse().unwrap_or(usize::MAX)),
            (Kind::Model, Some(v)) if !v.is_empty() && !self.session.model_available(&v) => {
                self.restore_draft();
                self.error(format!("Model `{v}` is unavailable"));
            }
            (Kind::Model, Some(v)) => {
                let (_, effort) = self.session.model();
                let cfg = self.session.config();
                let efforts = cfg.model_efforts(&v);
                if efforts.is_empty() {
                    self.restore_draft();
                    if let Err(e) = self.session.select_model(&v, "", true) {
                        self.error(format!("Can't save the model: {e}"));
                    }
                } else {
                    let items = efforts.iter().map(|e| item(e, "", e)).collect();
                    let target = if efforts.iter().any(|e| e == &effort) {
                        effort
                    } else {
                        cfg.fallback_effort(&v)
                    };
                    let ep = Picker::new(Kind::Effort(v.clone()), items);
                    self.open_modal(ep).select(&target);
                }
            }
            (Kind::Effort(model), Some(v)) => {
                if let Err(e) = self.session.select_model(model, &v, true) {
                    self.error(format!("Can't save the model: {e}"));
                }
            }
            (Kind::Task, Some(v)) => {
                if let Some(t) = self.session.task(&v) {
                    self.history_back = None;
                    self.history_items.clear();
                    self.panel = Some(Panel::markdown(task_text(&t)));
                }
            }
            (Kind::History, Some(v)) => {
                if let Ok(idx) = v.parse::<usize>()
                    && let Some(it) = self.history_items.get(idx)
                {
                    let panel = crate::ui::panel::history_panel(it);
                    let search = std::mem::take(&mut self.editor);
                    self.history_back = Some((p, search));
                    self.panel = Some(panel);
                } else {
                    self.restore_draft();
                }
            }
            (Kind::LuaPicker(id), v) => {
                let _ = self.session.ui_reply(*id, v);
            }
            (Kind::Manage(pick), Some(v)) => {
                self.choose_manage(pick.clone(), v);
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
            .insert_atom(&format!("@{v}"), AtomKind::File(crate::kit::files::path(v)));
        self.editor.insert(" ");
    }
}

impl App {
    pub fn open_models(&mut self) {
        let config = self.session.config();
        let (current, _) = self.session.model();
        let names = self.session.available_models();
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

pub(crate) fn history_row(index: usize, it: &tuse_core::HistoryItem) -> Item {
    item(it.role, it.summary.as_str(), index.to_string()).lead(index.to_string())
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

fn task_text(t: &tuse_core::TaskInfo) -> String {
    let elapsed = tuse_core::elapsed_since(&t.started, &t.ended)
        .map(crate::kit::rich::render::duration)
        .unwrap_or_default();
    let code = if t.code.trim().is_empty() {
        "(source unavailable)".to_string()
    } else {
        t.code.clone()
    };
    let ended = if t.ended.is_empty() { "-" } else { &t.ended };
    let log = tuse_core::Tasks::tail(&t.log, 200);
    format!(
        "**{}** `{}` · {} · {elapsed}\n\nstarted {} · ended {ended}\n\n```lua\n{code}\n```\n\n```text\n{log}\n```",
        t.name, t.id, t.status, t.started
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_detail_repeats_the_name_only_under_a_display_name() {
        let opus = ModelDef {
            window: 200_000,
            ..Default::default()
        };
        let sonnet = ModelDef {
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
        let docs = tuse_core::docs(Some("usage"));
        for (name, _) in crate::ui::picker::COMMANDS {
            assert!(docs.contains(&format!("`/{name}`")), "docs misses /{name}");
            assert!(tuse_config::BUILTIN_COMMANDS.contains(&name), "/{name}");
        }
        assert_eq!(
            crate::ui::picker::COMMANDS.len(),
            tuse_config::BUILTIN_COMMANDS.len()
        );
    }
}
