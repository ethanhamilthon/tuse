use crate::app::{App, QuestionPrompt};
use crate::panel::Panel;
use crate::picker::{Kind, Picker, item};

impl App {
    pub(crate) fn searching(&self) -> bool {
        self.picker.as_ref().is_some_and(|p| p.kind.is_searchable())
    }

    pub(crate) fn input_disabled(&self) -> bool {
        self.picker
            .as_ref()
            .is_some_and(|p| matches!(p.kind, Kind::LuaPicker(_)))
    }

    pub(crate) fn answering(&self) -> bool {
        self.question.is_some()
    }

    pub(crate) fn visible_panel_mut(&mut self) -> Option<&mut Panel> {
        if self.picker.is_some() {
            return None;
        }
        self.question
            .as_mut()
            .map(|q| &mut q.panel)
            .or(self.panel.as_mut())
    }

    pub(crate) fn open_modal(&mut self, mut p: Picker) -> &mut Picker {
        let same = self.picker.as_ref().is_some_and(|q| q.kind == p.kind);
        if self.saved_editor.is_none() {
            self.saved_editor = Some(std::mem::take(&mut self.editor));
        } else if !same {
            self.editor.clear();
        }
        if p.kind.is_searchable() {
            p.filter(&self.editor.text());
        }
        self.picker.insert(p)
    }

    pub(crate) fn open_lua_picker(&mut self, id: u64, options: Vec<String>) {
        self.question = None;
        self.panel = None;
        self.trace_back = None;
        self.trace_items.clear();
        let items = options
            .into_iter()
            .map(|s| item(s.clone(), "", s))
            .collect();
        self.open_modal(Picker::new(Kind::LuaPicker(id), items));
    }

    pub(crate) fn open_question(&mut self, id: u64, text: String) {
        self.picker = None;
        self.panel = None;
        self.trace_back = None;
        self.trace_items.clear();
        if self.saved_editor.is_none() {
            self.saved_editor = Some(std::mem::take(&mut self.editor));
        } else {
            self.editor.clear();
        }
        self.question = Some(QuestionPrompt {
            id,
            panel: Panel::markdown_unlinked(text),
        });
    }

    pub(crate) fn close_modal(&mut self) {
        self.picker = None;
        self.restore_draft();
    }

    pub(crate) fn restore_draft(&mut self) {
        if let Some(draft) = self.saved_editor.take() {
            self.editor = draft;
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::app::App;
    use crate::picker::{Kind, Picker, item};
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    fn press(app: &mut App, code: KeyCode, mods: KeyModifiers) {
        app.key(KeyEvent::new(code, mods), 10);
    }

    fn labels(app: &App) -> Vec<String> {
        let p = app.picker.as_ref().unwrap();
        p.shown.iter().map(|&i| p.items[i].label.clone()).collect()
    }

    fn type_text(app: &mut App, s: &str) {
        for c in s.chars() {
            press(app, KeyCode::Char(c), KeyModifiers::NONE);
        }
    }

    #[test]
    fn search_is_edited_in_the_editor_and_esc_restores_the_draft() {
        let mut app = crate::app::test_app("modal-search");
        app.paste("my draft");
        let items = ["alpha beta", "gallery", "/trace +lua"]
            .map(|s| item(s, "", s))
            .to_vec();
        app.open_modal(Picker::new(Kind::Resume, items));
        assert!(app.editor.is_empty());

        type_text(&mut app, "alpha be");
        assert_eq!(app.editor.text(), "alpha be");
        assert_eq!(labels(&app), ["alpha beta"]);
        press(&mut app, KeyCode::Char('w'), KeyModifiers::CONTROL);
        assert_eq!(app.editor.text(), "alpha ");
        press(&mut app, KeyCode::Backspace, KeyModifiers::NONE);
        press(&mut app, KeyCode::Char('u'), KeyModifiers::CONTROL);
        type_text(&mut app, "gal");
        assert_eq!(app.editor.text(), "gal");
        assert_eq!(labels(&app), ["gallery"]);

        press(&mut app, KeyCode::Enter, KeyModifiers::SHIFT);
        press(&mut app, KeyCode::End, KeyModifiers::NONE);
        press(&mut app, KeyCode::Char('u'), KeyModifiers::CONTROL);
        app.paste("/trace\n+lu");
        assert_eq!(app.editor.text(), "/trace +lu");
        assert_eq!(app.picker.as_ref().unwrap().kind, Kind::Resume);
        assert_eq!(labels(&app), ["/trace +lua"]);
        press(&mut app, KeyCode::Char('u'), KeyModifiers::CONTROL);
        press(&mut app, KeyCode::Char('c'), KeyModifiers::CONTROL);
        assert!(!app.quit && app.picker.is_some());

        press(&mut app, KeyCode::Esc, KeyModifiers::NONE);
        assert!(app.picker.is_none() && app.saved_editor.is_none());
        assert_eq!(app.editor.text(), "my draft");
        crate::app::cleanup(app);
    }

    #[test]
    fn model_to_effort_keeps_the_draft_and_a_refresh_keeps_the_search() {
        let mut app = crate::app::test_app("modal-models");
        app.paste("draft");
        app.open_models();
        let (current, _) = app.session.model();
        type_text(&mut app, "zzz-none");
        assert!(app.picker.as_ref().unwrap().shown.is_empty());
        app.handle_event(tuse_core::Event::Models);
        assert_eq!(app.editor.text(), "zzz-none");
        assert!(app.picker.as_ref().unwrap().shown.is_empty());

        press(&mut app, KeyCode::Char('u'), KeyModifiers::CONTROL);
        app.paste(&current);
        app.handle_event(tuse_core::Event::Models);
        assert_eq!(app.editor.text(), current);
        assert_eq!(
            app.picker.as_ref().unwrap().current().unwrap().value,
            current
        );

        press(&mut app, KeyCode::Enter, KeyModifiers::NONE);
        assert!(matches!(app.picker.as_ref().unwrap().kind, Kind::Effort(_)));
        assert!(app.editor.is_empty());
        press(&mut app, KeyCode::Enter, KeyModifiers::NONE);
        assert!(app.picker.is_none());
        assert_eq!(app.editor.text(), "draft");
        crate::app::cleanup(app);
    }

    #[test]
    fn lua_picker_disables_editor_question_uses_separate_input_and_pinned_stays_last() {
        let mut app = crate::app::test_app("modal-lua-ui");
        app.paste("keep draft");

        app.handle_event(tuse_core::Event::UiMarkdown {
            name: "p".into(),
            text: "PINNED".into(),
            pin: true,
            removed: false,
        });
        app.handle_event(tuse_core::Event::UiMarkdown {
            name: "u".into(),
            text: "UNPIN1".into(),
            pin: false,
            removed: false,
        });
        app.handle_event(tuse_core::Event::UiMarkdown {
            name: "u".into(),
            text: "UNPIN2".into(),
            pin: false,
            removed: false,
        });
        app.notice("LATER");
        let (lines, _) = app.chat.window(60, &app.palette, 0, 20);
        let rendered: Vec<String> = lines
            .iter()
            .map(|l| l.iter().map(|s| s.text.as_str()).collect::<String>())
            .filter(|s| !s.trim().is_empty())
            .collect();
        assert_eq!(rendered.last().unwrap(), "◆ PINNED", "{rendered:?}");
        assert!(
            rendered.iter().any(|l| l == "◆ UNPIN2")
                && !rendered.iter().any(|l| l.contains("UNPIN1"))
        );

        app.open_lua_picker(1, vec!["one".into(), "two".into()]);
        assert!(app.input_disabled() && app.editor.is_empty());
        type_text(&mut app, "two");
        app.paste("ignored");
        assert!(app.editor.is_empty());
        press(&mut app, KeyCode::Down, KeyModifiers::NONE);
        assert_eq!(app.picker.as_ref().unwrap().current().unwrap().value, "two");
        press(&mut app, KeyCode::Esc, KeyModifiers::NONE);
        assert!(app.picker.is_none() && app.editor.text() == "keep draft");

        app.open_question(2, "## Pick a name".into());
        assert!(app.answering() && app.editor.is_empty());
        type_text(&mut app, "my answer");
        assert_eq!(app.editor.text(), "my answer");
        press(&mut app, KeyCode::Enter, KeyModifiers::NONE);
        assert!(app.question.is_none() && !app.session.busy());
        assert_eq!(app.editor.text(), "keep draft");
        crate::app::cleanup(app);
    }

    #[test]
    fn trace_picker_opens_detail_panel_and_esc_preserves_selection() {
        let mut app = crate::app::test_app("modal-trace");
        app.paste("draft");
        app.trace_items = vec![
            tuse_core::TraceItem {
                role: "system",
                summary: "You are Tuse".into(),
                text: "# Heading\n\nBody".into(),
            },
            tuse_core::TraceItem {
                role: "lua",
                summary: "local x = 1".into(),
                text: "local x = 1\nlocal s = [[\nline1\nline2\n]]\nreturn x".into(),
            },
            tuse_core::TraceItem {
                role: "output",
                summary: "exit 0".into(),
                text: "exit 0\n\nok".into(),
            },
        ];
        let rows: Vec<_> = app
            .trace_items
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
        app.open_modal(Picker::new(Kind::Trace, rows));
        assert_eq!(labels(&app)[0], "0  system  You are Tuse");
        assert_eq!(labels(&app)[1], "1  lua  local x = 1");

        press(&mut app, KeyCode::Down, KeyModifiers::NONE);
        assert_eq!(app.picker.as_ref().unwrap().selected, 1);
        press(&mut app, KeyCode::Enter, KeyModifiers::NONE);
        assert!(app.picker.is_none() && app.panel.is_some());
        let panel_lines = app.panel.as_mut().unwrap().lines(60, 16, &app.palette);
        let body: String = panel_lines
            .iter()
            .flatten()
            .map(|s| s.text.as_str())
            .collect();
        assert!(body.contains("line1") && body.contains("line2"), "{body}");
        assert!(
            panel_lines
                .iter()
                .flatten()
                .any(|s| s.text == "local" && s.style.fg == app.palette.err),
            "lua keyword should be syntax highlighted"
        );

        press(&mut app, KeyCode::Esc, KeyModifiers::NONE);
        assert!(app.panel.is_none() && app.picker.as_ref().unwrap().selected == 1);
        press(&mut app, KeyCode::Esc, KeyModifiers::NONE);
        assert!(app.picker.is_none() && app.editor.text() == "draft");
        crate::app::cleanup(app);
    }

    #[test]
    fn slash_autocomplete_includes_custom_commands_and_updates_on_reload() {
        let mut app = crate::app::test_app("modal-custom-cmd");
        let cfg = app.cwd.join("tuse.lua");
        std::fs::write(
            &cfg,
            "return { commands = { hello_ui = function() require('tuse').show('h', 'hi', false) end } }",
        )
        .unwrap();
        app.run_command("reload");
        type_text(&mut app, "/hello");
        assert_eq!(labels(&app), ["/hello_ui"]);
        press(&mut app, KeyCode::Esc, KeyModifiers::NONE);
        app.editor.clear();
        std::fs::write(&cfg, "return { commands = { fresh_cmd = function() end } }").unwrap();
        app.run_command("reload");
        type_text(&mut app, "/");
        let all = labels(&app);
        assert!(all.contains(&"/fresh_cmd".to_string()) && !all.contains(&"/hello_ui".to_string()));
        crate::app::cleanup(app);
    }
}
