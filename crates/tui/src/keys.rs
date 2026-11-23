use crate::app::App;
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

impl App {
    pub fn picker_rows(&self) -> usize {
        self.list_rows.max(1)
    }

    pub fn key(&mut self, mut k: KeyEvent, page: usize) {
        let page = page.max(1);
        if k.kind == KeyEventKind::Release {
            return;
        }
        if k.modifiers
            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER)
            && let KeyCode::Char(c) = k.code
            && let Some(mapped) = crate::layout_keys::latin(c)
        {
            k.code = KeyCode::Char(mapped);
        }
        let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
        let alt = k.modifiers.contains(KeyModifiers::ALT);
        let shift = k.modifiers.contains(KeyModifiers::SHIFT);
        let primary = ctrl || k.modifiers.contains(KeyModifiers::SUPER);
        if primary
            && k.code == KeyCode::Char('c')
            && (!self.selection.text.is_empty() || self.editor.selection_range().is_some())
        {
            self.copy_selection(false);
            return;
        }
        self.selection = Default::default();
        let page_delta = match k.code {
            KeyCode::PageUp => -(page as isize),
            KeyCode::PageDown => page as isize,
            _ => 0,
        };
        if page_delta != 0 {
            if shift && let Some(panel) = self.visible_panel_mut() {
                panel.scroll_by(page_delta);
            } else {
                self.scroll_history(-page_delta);
            }
            return;
        }
        if self.picker.is_some() && self.picker_key(k, ctrl, shift) {
            return;
        }
        if self.question_key(k.code, shift || alt) {
            return;
        }
        if self.panel_key(k.code) {
            return;
        }
        let before = self.steering.map(|_| self.editor.buf.clone());
        let cols = self.input_cols;
        let e = &mut self.editor;
        if matches!(
            k.code,
            KeyCode::Left
                | KeyCode::Right
                | KeyCode::Up
                | KeyCode::Down
                | KeyCode::Home
                | KeyCode::End
        ) {
            if shift {
                e.anchor.get_or_insert(e.cursor);
            } else {
                e.clear_selection();
            }
        } else if matches!(k.code, KeyCode::Esc)
            || (ctrl && k.code == KeyCode::Char('e'))
            || (alt && matches!(k.code, KeyCode::Char('b' | 'f')))
        {
            e.clear_selection();
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
            KeyCode::Char('a') if primary => e.select_all(),
            KeyCode::Char('x') if primary => self.copy_selection(true),
            KeyCode::Char('e') if ctrl => e.end(),
            KeyCode::Char('u') if ctrl => e.kill_to_start(),
            KeyCode::Char('k') if ctrl => e.kill_to_end(),
            KeyCode::Char('w') if ctrl => e.delete_word(),
            KeyCode::Left if ctrl || alt => e.word_left(),
            KeyCode::Right if ctrl || alt => e.word_right(),
            KeyCode::Char('b') if alt => e.word_left(),
            KeyCode::Char('f') if alt => e.word_right(),
            KeyCode::Left => e.left(),
            KeyCode::Right => e.right(),
            KeyCode::Home => e.home(),
            KeyCode::End => e.end(),
            KeyCode::Backspace if alt || ctrl => e.delete_word(),
            KeyCode::Backspace => e.backspace(),
            KeyCode::Delete => e.delete(),
            KeyCode::Up => {
                e.vertical(true, cols);
            }
            KeyCode::Down => {
                e.vertical(false, cols);
            }
            KeyCode::Tab => {}
            KeyCode::Char(c) => e.insert(&c.to_string()),
            _ => {}
        }
        self.finish_edit(before);
    }

    fn copy_selection(&mut self, cut: bool) {
        let text = if !cut && !self.selection.text.is_empty() {
            Some(self.selection.text.clone())
        } else {
            self.editor.selected_text()
        };
        let Some(text) = text else {
            return;
        };
        match crate::clipboard::write(&text) {
            Ok(()) => {
                if cut {
                    self.editor.delete_selection();
                } else {
                    self.editor.clear_selection();
                }
                self.selection = Default::default();
                self.flash("Copied.");
            }
            Err(e) => self.error(e),
        }
    }
    fn question_key(&mut self, code: KeyCode, modified: bool) -> bool {
        let Some(q) = &mut self.question else {
            return false;
        };
        let step = self.palette.ui.scroll_step as isize;
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
            KeyCode::Up if self.editor.is_empty() => q.panel.scroll_by(-step),
            KeyCode::Down if self.editor.is_empty() => q.panel.scroll_by(step),
            _ => return false,
        }
        true
    }

    pub(super) fn finish_edit(&mut self, before: Option<Vec<char>>) {
        // The editor is a search box or answer input: queued messages stay queued and
        // `/`, `@`, `#` are plain text.
        if self.searching() {
            let q = self.editor.text();
            if let Some(p) = &mut self.picker {
                p.filter(&q);
            }
            return;
        }
        if self.answering() {
            return;
        }
        if before.is_some_and(|buf| buf != self.editor.buf)
            && let Some(id) = self.steering.take()
        {
            self.session.withdraw_input(id);
        }
        self.update_token_picker();
    }

    pub fn paste(&mut self, s: &str) {
        self.selection = Default::default();
        if self.input_disabled() {
            return;
        }
        if s.is_empty() {
            return self.paste_clipboard();
        }
        let before = self.steering.map(|_| self.editor.buf.clone());
        if self.searching() {
            self.editor.insert(&s.replace('\n', " "));
        } else {
            self.editor.insert(s);
        }
        self.finish_edit(before);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor::AtomKind;
    use crate::picker::Kind;
    use std::net::TcpListener;
    use std::time::Duration;
    use tuse_core::{Event, Options, Session, UserInput};

    #[test]
    fn only_picked_atoms_reach_the_hidden_context() {
        let mut app = crate::app::test_app("refs");
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
        crate::app::cleanup(app);
    }

    #[test]
    fn at_picks_files_by_absolute_path() {
        let mut app = crate::app::test_app("abs-at");
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
        crate::app::cleanup(app);
    }

    #[test]
    fn editing_withdraws_only_the_waiting_tui_message() {
        crate::app::test_home();
        // Hold the first request open so the worker cannot take the steering messages.
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let dir = std::env::temp_dir().join(format!("tuse-steering-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let cfg = dir.join("tuse.lua");
        let addr = listener.local_addr().unwrap();
        tuse_core::add_provider(&tuse_config::Connect {
            name: "steer".into(),
            base_url: format!("http://{addr}"),
            api_key: "test".into(),
        })
        .unwrap();
        tuse_core::set_state("model", "steer/m").unwrap();
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
        app.editor.cursor = 13;
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

        // An edit after the queue no longer contains the ID must survive a late User event.
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
        let mut app = crate::app::test_app("selection");
        let press = |app: &mut App, code, mods| app.key(KeyEvent::new(code, mods), 10);
        for primary in [KeyModifiers::CONTROL, KeyModifiers::SUPER] {
            app.paste("alpha beta");
            let word = KeyModifiers::SHIFT | KeyModifiers::ALT;
            press(&mut app, KeyCode::Left, word);
            assert_eq!(app.editor.selected_text().as_deref(), Some("beta"));
            press(&mut app, KeyCode::Char('c'), primary);
            crate::clipboard::TEST_TEXT.with(|s| assert_eq!(&*s.borrow(), "beta"));
            press(&mut app, KeyCode::Char('x'), primary);
            assert_eq!(app.editor.text(), "alpha beta");
            press(&mut app, KeyCode::Char('a'), primary);
            press(&mut app, KeyCode::Char('x'), primary);
            assert!(app.editor.is_empty());
            crate::clipboard::TEST_TEXT.with(|s| assert_eq!(&*s.borrow(), "alpha beta"));
            app.paste("clear");
            press(&mut app, KeyCode::Char('c'), primary);
            assert!(app.editor.is_empty() && !app.quit);
        }
        crate::app::cleanup(app);
    }

    #[test]
    fn russian_layout_preserves_typing_and_routes_modified_keys() {
        let mut app = crate::app::test_app("layout");
        let press = |app: &mut App, c, mods| app.key(KeyEvent::new(KeyCode::Char(c), mods), 10);
        press(&mut app, 'ф', KeyModifiers::NONE);
        assert_eq!(app.editor.text(), "ф");
        press(&mut app, 'ф', KeyModifiers::CONTROL);
        assert_eq!(app.editor.selected_text().as_deref(), Some("ф"));
        press(&mut app, 'С', KeyModifiers::CONTROL | KeyModifiers::SHIFT);
        crate::clipboard::TEST_TEXT.with(|s| assert_eq!(&*s.borrow(), "ф"));
        crate::app::cleanup(app);
    }

    #[test]
    fn streamed_text_is_not_repeated_after_a_tool_delta() {
        let mut app = crate::app::test_app("dup");
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
            .filter(|&i| matches!(app.chat.get_mut(i), Some(crate::chat::Entry::Assistant(_))))
            .count();
        assert_eq!(texts, 1);
        // Without streaming the final text still shows up once.
        app.handle_event(Event::Assistant {
            text: "Done.".into(),
        });
        assert!(
            matches!(app.chat.get_mut(app.chat.len() - 1), Some(crate::chat::Entry::Assistant(t)) if t == "Done.")
        );
        crate::app::cleanup(app);
    }

    #[test]
    fn enter_picks_a_folder_and_tab_completes_it() {
        let mut app = crate::app::test_app("enter-dir");
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
        crate::app::cleanup(app);
    }

    #[test]
    fn failed_new_keeps_the_session_and_its_next_message() {
        use std::io::{BufRead, BufReader, Read, Write};
        crate::app::test_home();
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
        tuse_core::add_provider(&tuse_config::Connect {
            name: "switch".into(),
            base_url: format!("http://{addr}"),
            api_key: "test".into(),
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
        session.set_model("switch/m", "default");
        let mut app = App::new(session.clone(), Some(cfg.clone()));
        std::fs::write(&cfg, "return { typo_key = 1 }").unwrap();
        app.run_command("new");
        assert!(std::sync::Arc::ptr_eq(&app.session, &session) && session.is_open());
        assert!(
            matches!(app.chat.get_mut(app.chat.len() - 1), Some(crate::chat::Entry::Error(e)) if e.contains("typo_key"))
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
