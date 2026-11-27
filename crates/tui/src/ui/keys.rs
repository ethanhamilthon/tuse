use crate::kit::editor::Edit;
use crate::ui::app::App;
use crate::ui::panel::history_panel;
use crate::ui::picker::Kind;
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

impl App {
    pub fn key(&mut self, mut k: KeyEvent, page: usize) {
        let page = page.max(1);
        if k.kind == KeyEventKind::Release {
            return;
        }
        if k.modifiers
            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER)
            && let KeyCode::Char(c) = k.code
            && let Some(mapped) = latin(c)
        {
            k.code = KeyCode::Char(mapped);
        }
        let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
        let alt = k.modifiers.contains(KeyModifiers::ALT);
        let shift = k.modifiers.contains(KeyModifiers::SHIFT);
        let primary = ctrl || k.modifiers.contains(KeyModifiers::SUPER);
        if primary && k.code == KeyCode::Char('c') && self.copy_selection(false) {
            return;
        }
        self.selection = Default::default();
        if primary && k.code == KeyCode::Char('a') {
            if !self.input_disabled() {
                self.editor.apply(Edit::SelectAll, self.input_cols);
            }
            return;
        }
        let page_delta = match k.code {
            KeyCode::PageUp => -(page as isize),
            KeyCode::PageDown => page as isize,
            _ => 0,
        };
        if page_delta != 0 {
            if !shift && self.picker.is_some() {
                let rows = self.picker_rows().max(1);
                if let Some(picker) = self.picker.as_mut() {
                    picker.page(page_delta.signum() * rows as isize, rows);
                }
            } else if shift && let Some(panel) = self.visible_panel_mut() {
                panel.scroll_by(page_delta);
                self.clear_link_hover();
            } else {
                self.scroll_history(-page_delta);
            }
            return;
        }
        if self.picker.is_some() && self.picker_key(k, ctrl, primary, shift) {
            return;
        }
        if self.question_key(k.code, shift || alt) {
            return;
        }
        if self.panel_key(k.code, k.modifiers) {
            return;
        }
        let before = self.steering.map(|_| self.editor.snapshot());
        let cols = self.input_cols;
        let e = &mut self.editor;
        let moves = matches!(
            k.code,
            KeyCode::Left
                | KeyCode::Right
                | KeyCode::Up
                | KeyCode::Down
                | KeyCode::Home
                | KeyCode::End
        );
        if moves {
            e.apply(if shift { Edit::Anchor } else { Edit::Deselect }, cols);
        } else if matches!(k.code, KeyCode::Esc)
            || (ctrl && k.code == KeyCode::Char('e'))
            || (alt && matches!(k.code, KeyCode::Char('b' | 'f')))
        {
            e.apply(Edit::Deselect, cols);
        }
        match k.code {
            KeyCode::Esc => {
                self.session.cancel();
                self.steering = None;
            }
            KeyCode::Enter if shift || alt => e.insert("\n"),
            KeyCode::Char('j') if ctrl => e.insert("\n"),
            KeyCode::Enter => self.submit(),
            KeyCode::Char('c') if primary => {
                if e.is_empty() {
                    self.quit = true;
                } else {
                    e.clear();
                }
            }
            KeyCode::Char('d') if ctrl && e.is_empty() => self.quit = true,
            KeyCode::Char('v') if ctrl || k.modifiers.contains(KeyModifiers::SUPER) => {
                return self.paste_clipboard();
            }
            KeyCode::Char('x') if primary => {
                self.copy_selection(true);
            }
            KeyCode::Char('e') if ctrl => e.apply(Edit::End, cols),
            KeyCode::Char('u') if ctrl => e.apply(Edit::KillToStart, cols),
            KeyCode::Char('k') if ctrl => e.apply(Edit::KillToEnd, cols),
            KeyCode::Char('w') if ctrl => e.apply(Edit::DeleteWord, cols),
            KeyCode::Left if ctrl || alt => e.apply(Edit::WordLeft, cols),
            KeyCode::Right if ctrl || alt => e.apply(Edit::WordRight, cols),
            KeyCode::Char('b') if alt => e.apply(Edit::WordLeft, cols),
            KeyCode::Char('f') if alt => e.apply(Edit::WordRight, cols),
            KeyCode::Left => e.apply(Edit::Left, cols),
            KeyCode::Right => e.apply(Edit::Right, cols),
            KeyCode::Home => e.apply(Edit::Home, cols),
            KeyCode::End => e.apply(Edit::End, cols),
            KeyCode::Backspace if alt || ctrl => e.apply(Edit::DeleteWord, cols),
            KeyCode::Backspace => e.apply(Edit::Backspace, cols),
            KeyCode::Delete => e.apply(Edit::Delete, cols),
            KeyCode::Up => e.apply(Edit::Up, cols),
            KeyCode::Down => e.apply(Edit::Down, cols),
            KeyCode::Tab => {}
            KeyCode::Char(c) => e.insert(&c.to_string()),
            _ => {}
        }
        self.finish_edit(before);
    }

    fn copy_selection(&mut self, cut: bool) -> bool {
        let text = if !cut && let Some(message) = self.selection.message {
            message.text(&self.chat_hits)
        } else if !self.input_disabled() {
            self.editor.selected_text()
        } else {
            None
        };
        let Some(text) = text else {
            return false;
        };
        match crate::kit::clipboard::write(&text) {
            Ok(()) => {
                if cut {
                    self.editor.apply(Edit::DeleteSelection, 1);
                } else {
                    self.editor.apply(Edit::Deselect, 1);
                }
                self.selection = Default::default();
                self.flash("Copied.");
            }
            Err(e) => self.error(e),
        }
        true
    }
    fn question_key(&mut self, code: KeyCode, modified: bool) -> bool {
        let Some(q) = &mut self.question else {
            return false;
        };
        let step = crate::kit::style::WHEEL_STEP as isize;
        match code {
            KeyCode::Esc => {
                let id = self.question.take().unwrap().id;
                self.restore_draft();
                let _ = self.session.ui_reply(id, None);
            }
            KeyCode::Enter if !modified => {
                let id = self.question.take().unwrap().id;
                let answer = self.editor.text();
                self.restore_draft();
                let _ = self.session.ui_reply(id, Some(answer));
            }
            KeyCode::Up if self.editor.is_empty() => {
                q.panel.scroll_by(-step);
                self.clear_link_hover();
            }
            KeyCode::Down if self.editor.is_empty() => {
                q.panel.scroll_by(step);
                self.clear_link_hover();
            }
            _ => return false,
        }
        true
    }
}

impl App {
    pub(crate) fn picker_key(
        &mut self,
        k: KeyEvent,
        ctrl: bool,
        primary: bool,
        shift: bool,
    ) -> bool {
        let rows = self.picker_rows();
        let Some(p) = &mut self.picker else {
            return false;
        };
        match k.code {
            KeyCode::Up | KeyCode::Down if shift && p.kind.is_token() => return false,
            KeyCode::Up => p.moved(-1, rows),
            KeyCode::Down => p.moved(1, rows),
            KeyCode::Char('p') if ctrl => p.moved(-1, rows),
            KeyCode::Char('n') if ctrl => p.moved(1, rows),
            KeyCode::Enter if !shift && p.kind == Kind::File => self.pick_file(),
            KeyCode::Enter if !shift && p.kind == Kind::Command && p.current().is_none() => {
                self.picker = None;
                return false;
            }
            KeyCode::Enter if !shift => self.choose(),
            KeyCode::Tab if !shift && !matches!(p.kind, Kind::LuaPicker(_)) => self.choose(),
            KeyCode::Esc => {
                let p = self.picker.take().unwrap();
                match &p.kind {
                    k if k.is_token() => self.dismissed = Some(self.editor.token().1),
                    Kind::LuaPicker(id) => {
                        let id = *id;
                        self.restore_draft();
                        let _ = self.session.ui_reply(id, None);
                    }
                    Kind::History => {
                        self.history_items.clear();
                        self.history_back = None;
                        self.restore_draft();
                    }
                    _ => self.restore_draft(),
                }
            }
            KeyCode::Char('x') if primary && p.kind == Kind::Task => {
                if let Some(it) = p.current() {
                    let id = it.value.clone();
                    let _ = self.session.stop_task(&id);
                    self.open_tasks();
                }
            }
            _ if p.kind.is_token() => return false,
            _ if !p.kind.is_searchable() => return true,
            _ => return !edits_text(k),
        }
        true
    }
}

fn latin(c: char) -> Option<char> {
    let c = c.to_lowercase().next()?;
    "йцукенгшщзфывапролдячсмить"
        .chars()
        .zip("qwertyuiopasdfghjklzxcvbnm".chars())
        .find_map(|(ru, en)| (c == ru).then_some(en))
}

fn edits_text(k: KeyEvent) -> bool {
    let primary = k
        .modifiers
        .intersects(KeyModifiers::CONTROL | KeyModifiers::SUPER);
    match k.code {
        KeyCode::Char(c) => !primary || matches!(c, 'a' | 'e' | 'u' | 'k' | 'w' | 'v' | 'x'),
        KeyCode::Backspace
        | KeyCode::Delete
        | KeyCode::Left
        | KeyCode::Right
        | KeyCode::Home
        | KeyCode::End => true,
        _ => false,
    }
}

impl App {
    pub(crate) fn panel_key(&mut self, code: KeyCode, mods: KeyModifiers) -> bool {
        if self.picker.is_some() || self.question.is_some() {
            return false;
        }
        let Some(panel) = &mut self.panel else {
            return false;
        };
        let step = crate::kit::style::WHEEL_STEP as isize;
        match code {
            KeyCode::Esc => {
                self.panel = None;
                self.clear_link_hover();
                if let Some((mut picker, search)) = self.history_back.take() {
                    picker.follow(self.picker_rows());
                    self.picker = Some(picker);
                    self.editor = search;
                }
            }
            KeyCode::Up if self.editor.is_empty() => {
                panel.scroll_by(-step);
                self.clear_link_hover();
            }
            KeyCode::Down if self.editor.is_empty() => {
                panel.scroll_by(step);
                self.clear_link_hover();
            }
            KeyCode::Left
                if self.editor.is_empty() && mods.is_empty() && self.history_back.is_some() =>
            {
                self.step_history(-1);
            }
            KeyCode::Right
                if self.editor.is_empty() && mods.is_empty() && self.history_back.is_some() =>
            {
                self.step_history(1);
            }
            _ => return false,
        }
        true
    }

    fn step_history(&mut self, delta: isize) {
        let rows = self.picker_rows();
        let Some((picker, _)) = &mut self.history_back else {
            return;
        };
        let Some(next) = picker.selected.checked_add_signed(delta) else {
            return;
        };
        let Some(&item_idx) = picker.shown.get(next) else {
            return;
        };
        let Some(history_idx) = picker
            .items
            .get(item_idx)
            .and_then(|it| it.value.parse::<usize>().ok())
        else {
            return;
        };
        let Some(it) = self.history_items.get(history_idx) else {
            return;
        };
        picker.selected = next;
        picker.follow(rows);
        self.panel = Some(history_panel(it));
        self.selection = Default::default();
        self.clear_link_hover();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kit::editor::AtomKind;
    use crate::ui::picker::Kind;
    use std::net::TcpListener;
    use std::time::Duration;
    use tuse_core::{Event, Options, Session, UserInput};

    #[test]
    fn only_picked_atoms_reach_the_hidden_context() {
        let mut app = crate::ui::app::test_app("refs");
        std::fs::write(app.cwd.join("a.txt"), "body").unwrap();
        std::fs::write(app.cwd.join("p.png"), "PNG").unwrap();
        app.editor.insert("#typed @b.txt ");
        app.editor
            .insert_atom("@a.txt", AtomKind::File("a.txt".into()));
        app.editor.insert(" ");
        app.editor.insert_image(app.cwd.join("p.png"));
        let input = tuse_core::build_input(&app.editor.text(), app.refs()).unwrap();
        assert_eq!(input.text, "#typed @b.txt @a.txt [Image #1]");
        let png = app.cwd.join("p.png").display().to_string();
        assert_eq!(
            input.context,
            format!("<image id=\"1\" path=\"{png}\"/>\n<file path=\"a.txt\">body</file>")
        );
        assert_eq!(input.images.len(), 1);
        crate::ui::app::cleanup(app);
    }

    #[test]
    fn at_picks_files_by_absolute_path() {
        let mut app = crate::ui::app::test_app("abs-at");
        let outside = std::env::temp_dir().join(format!("tuse-abs-at-{}", std::process::id()));
        std::fs::create_dir_all(outside.join("deep")).unwrap();
        std::fs::write(outside.join("deep/note.txt"), "far away").unwrap();
        let abs = outside.display().to_string();

        app.editor.insert(&format!("@{abs}/"));
        app.update_token_picker();
        let p = app.picker.as_ref().unwrap();
        assert_eq!(p.kind, Kind::File);
        assert_eq!(p.current().unwrap().value, format!("{abs}/deep/"));

        app.choose();
        assert_eq!(app.editor.text(), format!("@{abs}/deep/"));
        app.update_token_picker();
        app.editor.insert("no");
        app.update_token_picker();
        assert_eq!(
            app.picker.as_ref().unwrap().current().unwrap().value,
            format!("{abs}/deep/note.txt")
        );
        app.choose();
        let input = tuse_core::build_input(&app.editor.text(), app.refs()).unwrap();
        assert_eq!(
            input.context,
            format!("<file path=\"{abs}/deep/note.txt\">far away</file>")
        );
        std::fs::remove_dir_all(&outside).unwrap();
        crate::ui::app::cleanup(app);
    }

    #[test]
    fn editing_withdraws_only_the_waiting_tui_message() {
        crate::ui::app::test_home();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let dir = std::env::temp_dir().join(format!("tuse-steering-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let cfg = dir.join("tuse.lua");
        let addr = listener.local_addr().unwrap();
        tuse_core::Providers::add(&tuse_config::Connect {
            name: "steer".into(),
            base_url: format!("http://{addr}"),
            api_key: "test".into(),
            ..Default::default()
        })
        .unwrap();
        tuse_core::Prefs::set("model", "steer/m").unwrap();
        std::fs::write(&cfg, "return {}").unwrap();
        let session = Session::open(Options {
            cwd: dir.clone(),
            config: Some(cfg.clone()),
            mode: tuse_core::Mode::Tui,
            resume: None,
            start: Default::default(),
        })
        .unwrap();
        let mut app = App::new(session.clone(), Some(cfg));
        let press = |app: &mut App, code| app.key(KeyEvent::new(code, KeyModifiers::NONE), 0);
        app.editor.insert("draft /models after");
        app.editor.set_cursor(13);
        app.update_token_picker();
        assert_eq!(app.picker.as_ref().unwrap().kind, Kind::Command);
        press(&mut app, KeyCode::Enter);
        assert_eq!(app.picker.as_ref().unwrap().kind, Kind::Model);
        assert!(app.editor.is_empty());
        press(&mut app, KeyCode::Esc);
        assert!(app.picker.is_none());
        assert_eq!(app.editor.text(), "draft  after");
        app.editor.clear();
        let (first, _) = session.submit(UserInput::text("first"));
        loop {
            if matches!(app.events.recv_timeout(Duration::from_secs(5)).unwrap(),
                Event::User { id, .. } if id == first)
            {
                break;
            }
        }

        let (other, busy) = session.submit(UserInput::text("waiting"));
        assert!(busy);
        app.paste("waiting");
        press(&mut app, KeyCode::Enter);
        let waiting = app.steering.unwrap();
        press(&mut app, KeyCode::Left);
        assert_eq!(app.steering, Some(waiting));
        press(&mut app, KeyCode::End);
        press(&mut app, KeyCode::Char('!'));
        assert!(app.steering.is_none());
        assert_eq!(app.editor.text(), "waiting!");
        assert!(!session.withdraw_input(waiting));
        assert!(session.withdraw_input(other));
        assert!(session.busy());

        press(&mut app, KeyCode::Enter);
        let edited = app.steering.unwrap();
        assert_ne!(edited, waiting);
        assert!(session.withdraw_input(edited));

        app.paste("?");
        app.handle_event(Event::User {
            id: edited,
            text: "waiting!".into(),
            images: 0,
        });
        assert_eq!(app.editor.text(), "waiting!?");
        assert!(app.steering.is_none());

        session.close();
        drop(listener);
        std::fs::remove_dir_all(tuse_core::tuse_home().join(&session.id)).unwrap();
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn shortcuts_copy_cut_and_select_without_losing_input() {
        let mut app = crate::ui::app::test_app("selection");
        let press = |app: &mut App, code, mods| app.key(KeyEvent::new(code, mods), 10);
        for primary in [KeyModifiers::CONTROL, KeyModifiers::SUPER] {
            app.paste("alpha beta");
            let word = KeyModifiers::SHIFT | KeyModifiers::ALT;
            press(&mut app, KeyCode::Left, word);
            assert_eq!(app.editor.selected_text().as_deref(), Some("beta"));
            press(&mut app, KeyCode::Char('c'), primary);
            crate::kit::clipboard::TEST_TEXT.with(|s| assert_eq!(&*s.borrow(), "beta"));
            press(&mut app, KeyCode::Char('x'), primary);
            assert_eq!(app.editor.text(), "alpha beta");
            press(&mut app, KeyCode::Char('a'), primary);
            press(&mut app, KeyCode::Char('x'), primary);
            assert!(app.editor.is_empty());
            crate::kit::clipboard::TEST_TEXT.with(|s| assert_eq!(&*s.borrow(), "alpha beta"));
            app.paste("clear");
            press(&mut app, KeyCode::Char('c'), primary);
            assert!(app.editor.is_empty() && !app.quit);
        }
        crate::ui::app::cleanup(app);
    }

    #[test]
    fn russian_layout_preserves_typing_and_routes_modified_keys() {
        let mut app = crate::ui::app::test_app("layout");
        let press = |app: &mut App, c, mods| app.key(KeyEvent::new(KeyCode::Char(c), mods), 10);
        press(&mut app, 'ф', KeyModifiers::NONE);
        assert_eq!(app.editor.text(), "ф");
        press(&mut app, 'ф', KeyModifiers::CONTROL);
        assert_eq!(app.editor.selected_text().as_deref(), Some("ф"));
        press(&mut app, 'С', KeyModifiers::CONTROL | KeyModifiers::SHIFT);
        crate::kit::clipboard::TEST_TEXT.with(|s| assert_eq!(&*s.borrow(), "ф"));
        crate::ui::app::cleanup(app);
    }

    #[test]
    fn streamed_text_is_not_repeated_after_a_tool_delta() {
        let mut app = crate::ui::app::test_app("dup");
        let n = app.chat.len();
        app.handle_event(Event::Text("Let me check.".into()));
        app.handle_event(Event::ToolDelta {
            index: 0,
            id: "c1".into(),
            name: "lua".into(),
            code: "1".into(),
        });
        app.handle_event(Event::Assistant {
            text: "Let me check.".into(),
        });
        let texts = (n..app.chat.len())
            .filter(|&i| {
                matches!(
                    app.chat.get_mut(i),
                    Some(crate::kit::entry::Entry::Assistant(_))
                )
            })
            .count();
        assert_eq!(texts, 1);
        app.handle_event(Event::Assistant {
            text: "Done.".into(),
        });
        assert!(
            matches!(app.chat.get_mut(app.chat.len() - 1), Some(crate::kit::entry::Entry::Assistant(t)) if t == "Done.")
        );
        crate::ui::app::cleanup(app);
    }

    #[test]
    fn enter_picks_a_folder_and_tab_completes_it() {
        let mut app = crate::ui::app::test_app("enter-dir");
        std::fs::create_dir_all(app.cwd.join("some/inner")).unwrap();
        app.editor.insert("@some");
        app.update_token_picker();
        app.key(KeyEvent::from(KeyCode::Tab), 10);
        assert_eq!(app.editor.text(), "@some/");
        app.editor.clear();
        app.picker = None;
        app.editor.insert("@some");
        app.update_token_picker();
        app.key(KeyEvent::from(KeyCode::Enter), 10);
        assert_eq!(app.editor.text(), "@some/ ");
        assert!(app.picker.is_none());
        let refs = app.refs();
        assert_eq!(refs.files, [std::path::PathBuf::from("some/")]);
        crate::ui::app::cleanup(app);
    }

    #[test]
    fn failed_new_keeps_the_session_and_its_next_message() {
        use std::io::{BufRead, BufReader, Read, Write};
        crate::ui::app::test_home();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            for mut s in listener.incoming().flatten() {
                let mut r = BufReader::new(s.try_clone().unwrap());
                let mut len = 0;
                loop {
                    let mut line = String::new();
                    r.read_line(&mut line).unwrap();
                    if line.trim().is_empty() {
                        break;
                    }
                    if let Some(v) = line.to_lowercase().strip_prefix("content-length:") {
                        len = v.trim().parse().unwrap();
                    }
                }
                r.read_exact(&mut vec![0; len]).unwrap();
                let c = r#"{"choices":[{"delta":{"content":"still here"}}]}"#;
                let _ = write!(
                    s,
                    "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\ndata: {c}\n\ndata: [DONE]\n\n"
                );
            }
        });
        tuse_core::Providers::add(&tuse_config::Connect {
            name: "switch".into(),
            base_url: format!("http://{addr}"),
            api_key: "test".into(),
            ..Default::default()
        })
        .unwrap();
        let dir = std::env::temp_dir().join(format!("tuse-switch-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let cfg = dir.join("tuse.lua");
        std::fs::write(&cfg, "return {}").unwrap();
        let session = Session::open(Options {
            cwd: dir.clone(),
            config: Some(cfg.clone()),
            mode: tuse_core::Mode::Tui,
            resume: None,
            start: Default::default(),
        })
        .unwrap();
        session.set_model("switch/m", "medium");
        let mut app = App::new(session.clone(), Some(cfg.clone()));
        std::fs::write(&cfg, "return { bad = { typo_key = 1 } }").unwrap();
        app.run_command("new");
        assert!(std::sync::Arc::ptr_eq(&app.session, &session) && session.is_open());
        app.pump(&mut std::time::Instant::now());
        assert!(
            matches!(app.chat.get_mut(app.chat.len() - 1), Some(crate::kit::entry::Entry::Error(e)) if e.contains("typo_key"))
        );
        app.paste("hello");
        app.key(KeyEvent::from(KeyCode::Enter), 10);
        loop {
            match app.events.recv_timeout(Duration::from_secs(10)).unwrap() {
                Event::Assistant { text } => break assert_eq!(text, "still here"),
                Event::Error(e) => panic!("{e}"),
                _ => {}
            }
        }
        session.close();
        std::fs::remove_dir_all(tuse_core::tuse_home().join(&session.id)).ok();
        std::fs::remove_dir_all(dir).ok();
    }
}
