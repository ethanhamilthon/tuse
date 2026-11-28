use crate::kit::style::{Palette, Style};
use crate::kit::text::{Line, span};
use crate::ui::app::App;
use crate::ui::panel::Panel;
use crate::ui::picker::{Kind, ManagePick, Picker, item};
use tuse_core::{ConnectInput, Plugins, ProviderPatch, Providers};

pub const API_TYPES: [&str; 4] = [
    "openai-chat",
    "openai-responses",
    "anthropic-messages",
    "google-gemini",
];

#[derive(Debug, Clone)]
pub enum FormStep {
    ConnectBase {
        api: String,
    },
    ConnectKey {
        api: String,
        base: String,
    },
    ConnectName {
        api: String,
        base: String,
        key: String,
    },
    UpdateBase {
        name: String,
        api: String,
        current_base: String,
    },
    UpdateKey {
        name: String,
        api: String,
        base: Option<String>,
    },
    PluginAdd,
}

impl FormStep {
    fn secret(&self) -> bool {
        matches!(self, Self::ConnectKey { .. } | Self::UpdateKey { .. })
    }

    fn prompt(&self) -> String {
        match self {
            Self::ConnectBase { api } => format!(
                "**Connect provider** (`{api}`)\n\nStep 2/4 — Enter base URL (e.g. `https://api.openai.com/v1`):"
            ),
            Self::ConnectKey { api, base } => format!(
                "**Connect provider** (`{api}` · `{base}`)\n\nStep 3/4 — Enter API key or `$ENV_VAR` (masked, never recorded):"
            ),
            Self::ConnectName { api, base, .. } => format!(
                "**Connect provider** (`{api}` · `{base}`)\n\nStep 4/4 — Enter provider name:"
            ),
            Self::UpdateBase {
                name,
                api,
                current_base,
            } => format!(
                "**Update provider `{name}`** (`{api}`)\n\nCurrent base URL: `{current_base}`\nEnter new base URL (leave blank to keep current):"
            ),
            Self::UpdateKey { name, api, base } => {
                let shown = base.as_deref().unwrap_or("current");
                format!(
                    "**Update provider `{name}`** (`{api}` · `{shown}`)\n\nEnter new API key or `$ENV_VAR` (masked, leave blank to keep current):"
                )
            }
            Self::PluginAdd => "**Install plugin**\n\nEnter source (`owner/repo`, `owner/repo@ref`, `owner/repo/subdir`, or URL):".into(),
        }
    }
}

pub struct FormPrompt {
    pub step: FormStep,
    pub secret: bool,
    pub panel: Panel,
}

impl App {
    pub(crate) fn open_form(&mut self, step: FormStep) {
        self.picker = None;
        self.panel = None;
        self.question = None;
        self.history_back = None;
        self.history_items.clear();
        if self.saved_editor.is_none() {
            self.saved_editor = Some(std::mem::take(&mut self.editor));
        } else {
            self.editor.clear();
        }
        let (secret, prompt) = (step.secret(), step.prompt());
        self.form = Some(FormPrompt {
            step,
            secret,
            panel: Panel::markdown_unlinked(prompt),
        });
    }

    fn show_panel(&mut self, panel: Panel) {
        self.history_back = None;
        self.history_items.clear();
        self.panel = Some(panel);
    }

    fn open_arg_picker(&mut self, pick: ManagePick, pairs: &[(&str, &str)]) {
        let items = pairs.iter().map(|(n, d)| item(*n, *d, *n)).collect();
        self.open_modal(Picker::new(Kind::Manage(pick), items));
    }

    fn api_picker(&mut self, pick: ManagePick, title: String) -> &mut Picker {
        let items = API_TYPES.iter().map(|t| item(*t, "", *t)).collect();
        let mut picker = Picker::new(Kind::Manage(pick), items);
        picker.title = Some(title);
        self.open_modal(picker)
    }

    pub(crate) fn run_providers(&mut self, words: &[String]) -> bool {
        let w = (
            words.first().map(String::as_str),
            words.get(1).map(String::as_str),
            words.len(),
        );
        match w {
            (None, _, _) => {
                self.open_arg_picker(ManagePick::ProviderSub, &crate::ui::picker::PROVIDERS_ARGS)
            }
            (Some("list"), _, 1) => self.show_providers_list(),
            (Some("connect"), _, 1) => {
                self.api_picker(
                    ManagePick::ConnectApi,
                    "**Connect provider**\n\nStep 1/4 — Select API type:".into(),
                );
            }
            (Some("update"), _, 1) => self.start_provider_pick(ManagePick::ProviderUpdate),
            (Some("update"), Some(name), 2) => self.start_provider_update(name),
            (Some("disconnect"), _, 1) => self.start_provider_pick(ManagePick::ProviderDisconnect),
            (Some("disconnect"), Some(name), 2) => self.disconnect_provider(name),
            (Some(sub @ ("list" | "connect" | "update" | "disconnect")), _, _) => {
                self.error(format!("/providers {sub}: unexpected arguments"));
                return false;
            }
            (Some(other), _, _) => {
                self.error(format!(
                    "Unknown /providers subcommand `{other}` (expected list, connect, update, disconnect)"
                ));
                return false;
            }
        }
        true
    }

    pub(crate) fn run_plugins(&mut self, words: &[String]) -> bool {
        let w = (
            words.first().map(String::as_str),
            words.get(1).map(String::as_str),
            words.len(),
        );
        match w {
            (None, _, _) => {
                self.open_arg_picker(ManagePick::PluginSub, &crate::ui::picker::PLUGINS_ARGS)
            }
            (Some("list"), _, 1) => self.show_plugins_list(),
            (Some("get"), _, 1) => self.start_plugin_pick(ManagePick::PluginGet, false),
            (Some("get"), Some(t), 2) => self.show_plugin_detail(t),
            (Some("add"), _, 1) => self.open_form(FormStep::PluginAdd),
            (Some("add"), Some(s), 2) => self.add_plugin(s),
            (Some("delete"), _, 1) => self.start_plugin_pick(ManagePick::PluginDelete, false),
            (Some("delete"), Some(t), 2) => self.delete_plugin(t),
            (Some("update"), _, 1) => self.start_plugin_pick(ManagePick::PluginUpdate, true),
            (Some("update"), Some(t), 2) => self.update_plugins(Some(t)),
            (Some(sub @ ("list" | "get" | "add" | "delete" | "update")), _, _) => {
                self.error(format!("/plugins {sub}: unexpected arguments"));
                return false;
            }
            (Some(other), _, _) => {
                self.error(format!(
                    "Unknown /plugins subcommand `{other}` (expected list, get, add, delete, update)"
                ));
                return false;
            }
        }
        true
    }

    pub(crate) fn choose_manage(&mut self, pick: ManagePick, value: String) {
        match pick {
            ManagePick::ProviderSub => {
                self.run_providers(&[value]);
            }
            ManagePick::PluginSub => {
                self.run_plugins(&[value]);
            }
            ManagePick::ConnectApi => self.open_form(FormStep::ConnectBase { api: value }),
            ManagePick::ProviderUpdate => self.start_provider_update(&value),
            ManagePick::UpdateApi { name, current_base } => {
                self.open_form(FormStep::UpdateBase {
                    name,
                    api: value,
                    current_base,
                });
            }
            ManagePick::ProviderDisconnect => self.disconnect_provider(&value),
            ManagePick::PluginGet => self.show_plugin_detail(&value),
            ManagePick::PluginDelete => self.delete_plugin(&value),
            ManagePick::PluginUpdate => {
                self.update_plugins((!value.is_empty()).then_some(value.as_str()));
            }
        }
    }

    pub(crate) fn submit_form(&mut self) {
        let Some(form) = self.form.take() else { return };
        let val = self.editor.text().trim().to_string();
        self.editor.clear();
        match form.step {
            FormStep::ConnectBase { api } if val.is_empty() => {
                self.error("Base URL is required");
                self.open_form(FormStep::ConnectBase { api });
            }
            FormStep::ConnectBase { api } => {
                self.open_form(FormStep::ConnectKey { api, base: val });
            }
            FormStep::ConnectKey { api, base } if val.is_empty() => {
                self.error("API key is required");
                self.open_form(FormStep::ConnectKey { api, base });
            }
            FormStep::ConnectKey { api, base } => {
                self.open_form(FormStep::ConnectName {
                    api,
                    base,
                    key: val,
                });
            }
            FormStep::ConnectName { api, base, key } if val.is_empty() => {
                self.error("Provider name is required");
                self.open_form(FormStep::ConnectName { api, base, key });
            }
            FormStep::ConnectName { api, base, key } => {
                self.restore_draft();
                self.manage_job(move |s| {
                    let r = s.connect_provider(ConnectInput {
                        name: val,
                        api: Some(api),
                        base,
                        key,
                        headers: None,
                        replace: false,
                    })?;
                    Ok(format!(
                        "Connected {} ({}, {} models).",
                        r.name, r.kind, r.models
                    ))
                });
            }
            FormStep::UpdateBase {
                name,
                api,
                current_base: _,
            } => {
                self.open_form(FormStep::UpdateKey {
                    name,
                    api,
                    base: (!val.is_empty()).then_some(val),
                });
            }
            FormStep::UpdateKey { name, api, base } => {
                self.restore_draft();
                self.manage_job(move |s| {
                    let r = s.update_provider(
                        &name,
                        ProviderPatch {
                            api: Some(api),
                            base,
                            key: (!val.is_empty()).then_some(val),
                            headers: None,
                        },
                    )?;
                    Ok(format!(
                        "Updated {} ({}, {} models).",
                        r.name, r.kind, r.models
                    ))
                });
            }
            FormStep::PluginAdd => {
                self.restore_draft();
                if val.is_empty() {
                    self.error("Plugin source is required");
                } else {
                    self.add_plugin(&val);
                }
            }
        }
    }

    fn show_providers_list(&mut self) {
        match Providers::views() {
            Ok(list) => {
                let rows = list.into_iter().map(|v| (v.name, v.kind, v.base)).collect();
                self.show_panel(cols_panel("Providers", rows, &self.palette));
            }
            Err(e) => self.error(e),
        }
    }

    fn start_provider_pick(&mut self, pick: ManagePick) {
        let list = match Providers::views() {
            Ok(l) => l,
            Err(e) => return self.error(e),
        };
        if list.is_empty() {
            return self.flash("No providers connected.");
        }
        let items = list
            .into_iter()
            .map(|p| item(p.name.clone(), format!("{} · {}", p.kind, p.base), p.name))
            .collect();
        self.open_modal(Picker::new(Kind::Manage(pick), items));
    }

    fn start_provider_update(&mut self, name: &str) {
        let existing = match Providers::get(name) {
            Ok(Some(c)) => c,
            Ok(None) => return self.error(format!("provider `{name}` not found")),
            Err(e) => return self.error(e),
        };
        let current = existing.api_type().to_string();
        let title = format!(
            "**Update provider `{}`**\n\nSelect API type (current: `{current}`):",
            existing.name
        );
        self.api_picker(
            ManagePick::UpdateApi {
                name: existing.name,
                current_base: existing.base_url,
            },
            title,
        )
        .select(&current);
    }

    fn disconnect_provider(&mut self, name: &str) {
        match self.session.disconnect_provider(name) {
            Ok(removed) => self.flash(format!("Disconnected {removed}.")),
            Err(e) => self.error(e),
        }
    }

    fn show_plugins_list(&mut self) {
        match Plugins::list() {
            Ok(list) => {
                let rows = list
                    .into_iter()
                    .map(|v| {
                        (
                            v.name,
                            v.version,
                            v.source.unwrap_or_else(|| "(local)".into()),
                        )
                    })
                    .collect();
                self.show_panel(cols_panel("Plugins", rows, &self.palette));
            }
            Err(e) => self.error(e),
        }
    }

    fn show_plugin_detail(&mut self, target: &str) {
        match Plugins::get(target) {
            Ok(detail) => {
                let manifest = serde_json::to_string_pretty(&detail.manifest).unwrap_or_default();
                let source = detail.source.as_deref().unwrap_or("(local)");
                let md = format!(
                    "**{}** `{}`\n\nsource: `{source}`\n\npath: `{}`\n\nentrypoint: `{}`\n\n```json\n{manifest}\n```",
                    detail.name, detail.version, detail.path, detail.entrypoint
                );
                self.show_panel(Panel::markdown(md));
            }
            Err(e) => self.error(e),
        }
    }

    fn start_plugin_pick(&mut self, pick: ManagePick, include_all: bool) {
        let list = match Plugins::list() {
            Ok(l) => l,
            Err(e) => return self.error(e),
        };
        if list.is_empty() {
            return self.flash("No plugins installed.");
        }
        let mut items = vec![];
        if include_all {
            items.push(item("(all)", "update every installed plugin", ""));
        }
        for pl in list {
            let source = pl.source.as_deref().unwrap_or("(local)");
            items.push(item(
                pl.name.clone(),
                format!("{} · {source}", pl.version),
                pl.name,
            ));
        }
        self.open_modal(Picker::new(Kind::Manage(pick), items));
    }

    fn manage_job(
        &mut self,
        work: impl FnOnce(&tuse_core::Session) -> Result<String, String> + Send + 'static,
    ) {
        use std::sync::atomic::Ordering;
        let busy = self.management_busy.clone();
        if busy.swap(true, Ordering::AcqRel) {
            self.flash("A management operation is already running.");
            return;
        }
        let origin = self.session.clone();
        std::thread::spawn(move || {
            let result = work(&origin);
            busy.store(false, Ordering::Release);
            match result {
                Ok(message) => {
                    origin.flash(message);
                }
                Err(error) => origin.error(error),
            }
        });
    }

    fn add_plugin(&mut self, source: &str) {
        let source = source.to_string();
        self.manage_job(move |_| {
            let res = Plugins::add(&source, false)?;
            Ok(format!(
                "Installed {} {}. Run /reload to activate eligible plugins.",
                res.name, res.version
            ))
        });
    }

    fn delete_plugin(&mut self, target: &str) {
        match Plugins::delete(target) {
            Ok(name) => self.flash(format!("Removed {name}. Run /reload to apply.")),
            Err(e) => self.error(e),
        }
    }

    fn update_plugins(&mut self, target: Option<&str>) {
        let target = target.map(str::to_string);
        self.manage_job(move |_| {
            let list = Plugins::update(target.as_deref())?;
            Ok(format!(
                "Updated {} plugin(s). Run /reload to apply.",
                list.len()
            ))
        });
    }
}

fn cols_panel(title: &str, rows: Vec<(String, String, String)>, p: &Palette) -> Panel {
    let (head, fg, info, dim) = (
        Style::fg(p.accent).bold(),
        Style::fg(p.fg),
        Style::fg(p.heading),
        Style::fg(p.muted),
    );
    let mut lines: Vec<Line> = vec![vec![span(title, head)]];
    if rows.is_empty() {
        lines.push(vec![span("  (none)", dim)]);
    } else {
        let nw = rows
            .iter()
            .map(|r| crate::kit::text::width(&r.0))
            .max()
            .unwrap_or(0);
        let sw = rows
            .iter()
            .map(|r| crate::kit::text::width(&r.1))
            .max()
            .unwrap_or(0);
        for (a, b, c) in rows {
            lines.push(vec![
                span(format!("  {a:<nw$}  "), fg),
                span(format!("{b:<sw$}  "), info),
                span(c, dim),
            ]);
        }
    }
    Panel::styled(lines)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use flate2::Compression;
    use flate2::write::GzEncoder;
    use std::io::{BufRead, BufReader, Write};
    use std::net::TcpListener;

    fn press(app: &mut App, code: KeyCode, mods: KeyModifiers) {
        app.key(KeyEvent::new(code, mods), 10);
    }

    fn type_text(app: &mut App, s: &str) {
        for c in s.chars() {
            press(app, KeyCode::Char(c), KeyModifiers::NONE);
        }
    }

    fn mock_provider_server() -> String {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", l.local_addr().unwrap());
        std::thread::spawn(move || {
            for mut s in l.incoming().flatten() {
                let mut r = BufReader::new(s.try_clone().unwrap());
                let mut line = String::new();
                let mut auth = String::new();
                while r.read_line(&mut line).unwrap_or(0) > 0 && !line.trim().is_empty() {
                    if line.to_ascii_lowercase().starts_with("authorization:") {
                        auth = line.clone();
                    }
                    line.clear();
                }
                let (status, body) = if auth.contains("sk-bad-secret") {
                    ("401 Unauthorized", r#"{"error":"invalid sk-bad-secret"}"#)
                } else {
                    ("200 OK", r#"{"data":[{"id":"gpt-test"}]}"#)
                };
                let _ = write!(
                    s,
                    "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
            }
        });
        url
    }

    fn serve_plugin(name: &str, version: &str) -> String {
        let mut enc = GzEncoder::new(Vec::new(), Compression::default());
        {
            let mut tar = tar::Builder::new(&mut enc);
            for (path, body) in [
                (
                    "pkg/tuse-plugin.json",
                    format!(r#"{{"name":"{name}","version":"{version}"}}"#),
                ),
                ("pkg/init.lua", "return { prompts = { p = 'ok' } }".into()),
            ] {
                let mut h = tar::Header::new_gnu();
                h.set_size(body.len() as u64);
                h.set_mode(0o644);
                h.set_cksum();
                tar.append_data(&mut h, path, body.as_bytes()).unwrap();
            }
            tar.finish().unwrap();
        }
        let body = enc.finish().unwrap();
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", l.local_addr().unwrap());
        std::thread::spawn(move || {
            for mut s in l.incoming().flatten() {
                let mut r = BufReader::new(s.try_clone().unwrap());
                let mut line = String::new();
                while r.read_line(&mut line).unwrap_or(0) > 0 && !line.trim().is_empty() {
                    line.clear();
                }
                let hdr = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/gzip\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                let _ = s.write_all(hdr.as_bytes());
                let _ = s.write_all(&body);
            }
        });
        url
    }

    fn wait(app: &mut App, condition: impl Fn(&App) -> bool) {
        let end = std::time::Instant::now() + std::time::Duration::from_secs(5);
        let mut tick = std::time::Instant::now();
        loop {
            app.pump(&mut tick);
            if condition(app) {
                return;
            }
            assert!(
                std::time::Instant::now() < end,
                "management action timed out"
            );
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }

    fn run_isolated(test_name: &str) {
        static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let n = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!("tuse-tui-iso-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let out = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", test_name, "--nocapture"])
            .env("TUSE_TEST_ISOLATED", "1")
            .env("HOME", &root)
            .env("TUSE_HOME", root.join(".tuse"))
            .env_remove("TUSE_GLOBAL_CONFIG")
            .env_remove("TUSE_PLUGINS_URL")
            .output()
            .unwrap();
        let _ = std::fs::remove_dir_all(&root);
        assert!(
            out.status.success(),
            "isolated test {test_name} failed:\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
    }

    #[test]
    fn pending_management_leaves_input_responsive_and_stays_with_its_session() {
        let mut app = crate::ui::app::test_app("pending-management");
        let (entered_tx, entered) = std::sync::mpsc::channel();
        let (release, release_rx) = std::sync::mpsc::channel();
        app.manage_job(move |_| {
            entered_tx.send(()).unwrap();
            release_rx.recv().unwrap();
            Ok("old session result".into())
        });
        entered
            .recv_timeout(std::time::Duration::from_secs(2))
            .unwrap();
        type_text(&mut app, "draft");
        assert_eq!(app.editor.text(), "draft");
        let old_job = app.management_busy.clone();
        app.switch(None);
        type_text(&mut app, "new draft");
        release.send(()).unwrap();
        wait(&mut app, |_| {
            !old_job.load(std::sync::atomic::Ordering::Acquire)
        });
        assert_eq!(app.editor.text(), "new draft");
        assert!(
            app.flash
                .as_ref()
                .is_none_or(|(s, _)| s != "old session result")
        );
        crate::ui::app::cleanup(app);
    }

    #[test]
    fn providers_connect_masks_secret_never_leaks_and_esc_cancels_cleanly() {
        if std::env::var_os("TUSE_TEST_ISOLATED").is_none() {
            return run_isolated(
                "ui::manage::tests::providers_connect_masks_secret_never_leaks_and_esc_cancels_cleanly",
            );
        }
        let url = mock_provider_server();
        let cwd = std::env::temp_dir().join(format!("tuse-tui-prov-{}", std::process::id()));
        std::fs::create_dir_all(&cwd).unwrap();
        let s = tuse_core::Session::open(tuse_core::Options {
            cwd: cwd.clone(),
            mode: tuse_core::Mode::Tui,
            start: Default::default(),
            resume: None,
            flags: tuse_core::LoadFlags {
                no_global: true,
                no_plugins: true,
            },
        })
        .unwrap();
        let mut app = App::new(s);
        app.paste("keep-draft");

        let secret = "sk-super-top-secret-999";
        app.run_providers(&["connect".into()]);
        assert_eq!(
            app.picker.as_ref().unwrap().kind,
            Kind::Manage(ManagePick::ConnectApi)
        );
        press(&mut app, KeyCode::Enter, KeyModifiers::NONE);
        type_text(&mut app, &url);
        press(&mut app, KeyCode::Enter, KeyModifiers::NONE);
        assert!(app.secret_input());
        type_text(&mut app, secret);
        let frame = app.compose(60, 16);
        let screen_text: String = frame
            .screen
            .iter()
            .flatten()
            .map(|s| s.text.as_str())
            .collect();
        assert!(!screen_text.contains(secret));
        assert!(screen_text.contains(&"•".repeat(secret.len())));
        crate::kit::clipboard::TEST_TEXT.with(|s| s.borrow_mut().clear());
        press(&mut app, KeyCode::Char('a'), KeyModifiers::CONTROL);
        press(&mut app, KeyCode::Char('c'), KeyModifiers::CONTROL);
        crate::kit::clipboard::TEST_TEXT.with(|s| assert!(s.borrow().is_empty()));
        press(&mut app, KeyCode::Esc, KeyModifiers::NONE);
        assert!(app.form.is_none() && app.editor.text() == "keep-draft");
        assert!(Providers::get("demo_tui").unwrap().is_none());

        app.run_providers(&["connect".into()]);
        press(&mut app, KeyCode::Enter, KeyModifiers::NONE);
        type_text(&mut app, &url);
        press(&mut app, KeyCode::Enter, KeyModifiers::NONE);
        type_text(&mut app, "sk-bad-secret");
        press(&mut app, KeyCode::Enter, KeyModifiers::NONE);
        type_text(&mut app, "bad_tui");
        press(&mut app, KeyCode::Enter, KeyModifiers::NONE);
        wait(&mut app, |app| {
            app.session.records().iter().any(|r| {
                matches!(
                    r,
                    tuse_core::Record::Message {
                        level: tuse_core::Level::Error,
                        ..
                    }
                )
            })
        });
        assert!(Providers::get("bad_tui").unwrap().is_none());
        let err_dump = format!("{:?}", app.session.records());
        assert!(!err_dump.contains("sk-bad-secret"));

        app.run_providers(&["connect".into()]);
        press(&mut app, KeyCode::Enter, KeyModifiers::NONE);
        type_text(&mut app, &url);
        press(&mut app, KeyCode::Enter, KeyModifiers::NONE);
        type_text(&mut app, secret);
        press(&mut app, KeyCode::Enter, KeyModifiers::NONE);
        type_text(&mut app, "demo_tui");
        press(&mut app, KeyCode::Enter, KeyModifiers::NONE);
        wait(&mut app, |_| Providers::get("demo_tui").unwrap().is_some());
        assert_eq!(app.editor.text(), "keep-draft");

        let history_json = serde_json::to_string(&app.session.history()).unwrap();
        let ui_json = serde_json::to_string(&app.session.ui_snapshot()).unwrap();
        let snap_json = serde_json::to_string(&app.session.snapshot(true)).unwrap();
        assert!(!history_json.contains(secret));
        assert!(!ui_json.contains(secret));
        assert!(!snap_json.contains(secret));

        app.run_providers(&["list".into()]);
        let list_text: String = app
            .panel
            .as_mut()
            .unwrap()
            .lines(80, 16, &app.palette)
            .iter()
            .flatten()
            .map(|s| s.text.as_str())
            .collect();
        assert!(list_text.contains("demo_tui") && !list_text.contains(secret));
        press(&mut app, KeyCode::Esc, KeyModifiers::NONE);

        app.run_providers(&["update".into(), "demo_tui".into()]);
        press(&mut app, KeyCode::Down, KeyModifiers::NONE);
        press(&mut app, KeyCode::Enter, KeyModifiers::NONE);
        press(&mut app, KeyCode::Enter, KeyModifiers::NONE);
        press(&mut app, KeyCode::Enter, KeyModifiers::NONE);
        wait(&mut app, |_| {
            Providers::get("demo_tui").unwrap().unwrap().api == "openai-responses"
        });

        app.run_providers(&["disconnect".into(), "demo_tui".into()]);
        assert!(Providers::get("demo_tui").unwrap().is_none());

        app.editor.clear();
        type_text(&mut app, "/connect");
        press(&mut app, KeyCode::Enter, KeyModifiers::NONE);
        assert_eq!(app.editor.text(), "/connect");

        app.editor.clear();
        app.paste("/providers connect");
        app.picker = None;
        press(&mut app, KeyCode::Enter, KeyModifiers::NONE);
        assert_eq!(
            app.picker.as_ref().unwrap().kind,
            Kind::Manage(ManagePick::ConnectApi)
        );
        press(&mut app, KeyCode::Esc, KeyModifiers::NONE);
        assert!(app.picker.is_none());
        assert_eq!(app.editor.text(), "");

        app.paste("/providers connect");
        app.picker = None;
        press(&mut app, KeyCode::Enter, KeyModifiers::NONE);
        press(&mut app, KeyCode::Enter, KeyModifiers::NONE);
        type_text(&mut app, &url);
        press(&mut app, KeyCode::Enter, KeyModifiers::NONE);
        type_text(&mut app, secret);
        press(&mut app, KeyCode::Enter, KeyModifiers::NONE);
        type_text(&mut app, "submit_tui");
        press(&mut app, KeyCode::Enter, KeyModifiers::NONE);
        wait(&mut app, |_| {
            Providers::get("submit_tui").unwrap().is_some()
        });
        assert_eq!(app.editor.text(), "");

        app.paste("/providers update submit_tui");
        app.picker = None;
        press(&mut app, KeyCode::Enter, KeyModifiers::NONE);
        press(&mut app, KeyCode::Esc, KeyModifiers::NONE);
        assert_eq!(app.editor.text(), "");

        app.paste("/providers bogus");
        app.picker = None;
        press(&mut app, KeyCode::Enter, KeyModifiers::NONE);
        assert_eq!(app.editor.text(), "/providers bogus");

        app.editor.clear();
        let _ = Providers::disconnect("submit_tui");
        crate::ui::app::cleanup(app);
    }

    #[test]
    fn plugins_management_works_without_auto_reload_even_under_no_plugins() {
        if std::env::var_os("TUSE_TEST_ISOLATED").is_none() {
            return run_isolated(
                "ui::manage::tests::plugins_management_works_without_auto_reload_even_under_no_plugins",
            );
        }
        let url = serve_plugin("tuip", "1.0.0");
        unsafe {
            std::env::set_var("TUSE_PLUGINS_URL", &url);
        }
        let cwd = std::env::temp_dir().join(format!("tuse-tui-plugins-{}", std::process::id()));
        std::fs::create_dir_all(&cwd).unwrap();
        let s = tuse_core::Session::open(tuse_core::Options {
            cwd: cwd.clone(),
            mode: tuse_core::Mode::Tui,
            start: Default::default(),
            resume: None,
            flags: tuse_core::LoadFlags {
                no_global: true,
                no_plugins: true,
            },
        })
        .unwrap();
        let mut app = App::new(s);
        app.paste("/plugins add acme/tuip");
        app.picker = None;
        press(&mut app, KeyCode::Enter, KeyModifiers::NONE);
        assert_eq!(app.editor.text(), "");
        assert!(app.session.config().plugins.is_empty());
        wait(&mut app, |_| {
            Plugins::list().unwrap().iter().any(|p| p.name == "tuip")
        });

        app.paste("/plugins get tuip");
        app.picker = None;
        press(&mut app, KeyCode::Enter, KeyModifiers::NONE);
        assert_eq!(app.editor.text(), "");
        let detail_text: String = app
            .panel
            .as_mut()
            .unwrap()
            .lines(80, 16, &app.palette)
            .iter()
            .flatten()
            .map(|s| s.text.as_str())
            .collect();
        assert!(detail_text.contains("tuip") && detail_text.contains("1.0.0"));

        app.paste("/plugins delete tuip");
        app.picker = None;
        press(&mut app, KeyCode::Enter, KeyModifiers::NONE);
        assert_eq!(app.editor.text(), "");
        assert!(Plugins::list().unwrap().iter().all(|p| p.name != "tuip"));

        unsafe {
            std::env::remove_var("TUSE_PLUGINS_URL");
        }
        crate::ui::app::cleanup(app);
    }
}
