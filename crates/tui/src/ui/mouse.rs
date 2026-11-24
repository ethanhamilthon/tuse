use crate::kit::editor::Edit;
use crate::ui::app::App;
use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};

impl App {
    pub fn mouse(&mut self, m: MouseEvent) -> bool {
        let point = (m.row as usize, m.column as usize);
        let inside = self.input_area.is_some_and(|a| {
            point.0 >= a.row
                && point.0 < a.row + a.height
                && point.1 >= a.col
                && point.1 < a.col + a.cols
        });
        match m.kind {
            MouseEventKind::ScrollUp => {
                self.wheel(-1, point.0);
                true
            }
            MouseEventKind::ScrollDown => {
                self.wheel(1, point.0);
                true
            }
            MouseEventKind::Moved => {
                self.pointer = Some(point);
                let at = self.link_at(point);
                std::mem::replace(&mut self.hover, at) != at
            }
            MouseEventKind::Down(MouseButton::Left) => {
                self.editor.apply(Edit::Deselect, 1);
                self.selection = Default::default();
                if self.pointer != Some(point) {
                    self.hover = None;
                    self.pointer = None;
                }
                self.pressed_link = self.link_at(point).map(|id| (point, id));
                self.click(point.1, point.0);
                self.selection.drag = Some((point, point));
                if inside && !self.input_disabled() {
                    self.selection.input = Some(self.editor.cursor());
                }
                true
            }
            MouseEventKind::Drag(MouseButton::Left) | MouseEventKind::Up(MouseButton::Left) => {
                let pressed = if matches!(m.kind, MouseEventKind::Up(_)) {
                    self.pressed_link.take()
                } else {
                    None
                };
                if let Some((a, _)) = self.selection.drag {
                    if a != point {
                        self.clear_link_hover();
                    }
                    if inside && let Some(index) = self.selection.input {
                        self.click(point.1, point.0);
                        self.editor.apply(Edit::AnchorAt(index), 1);
                        self.selection.drag = Some((a, a));
                    } else {
                        self.editor.apply(Edit::Deselect, 1);
                        self.selection.drag = Some((a, point));
                    }
                    if matches!(m.kind, MouseEventKind::Up(_)) {
                        self.selection.input = None;
                        if self.selection.drag == Some((a, a)) {
                            self.selection.drag = None;
                            if a == point
                                && let Some((pt, id)) = pressed
                                && pt == point
                                && self.link_at(point) == Some(id)
                                && let Some(t) = crate::kit::links::target(id)
                            {
                                match t {
                                    crate::kit::links::LinkTarget::Url(u) => {
                                        self.session.open_url(&u)
                                    }
                                    crate::kit::links::LinkTarget::Path(p) => {
                                        self.session.open_path(&p)
                                    }
                                }
                            }
                        }
                    }
                }
                true
            }
            _ => false,
        }
    }

    pub(crate) fn link_at(&self, (row, col): (usize, usize)) -> Option<u32> {
        self.links
            .iter()
            .find(|l| l.row == row && col >= l.start && col < l.end)
            .map(|l| l.id)
    }

    fn wheel(&mut self, dir: isize, row: usize) {
        self.selection = Default::default();
        self.clear_link_hover();
        let step = self.palette.ui.scroll_step as isize;
        if self.overlay_rows.contains(&row) {
            if let Some(p) = &mut self.picker {
                let n = p.shown.len() as isize;
                if n > 0 {
                    p.selected = (p.selected as isize + dir).clamp(0, n - 1) as usize;
                    p.follow(self.list_rows);
                }
                return;
            }
            if let Some(panel) = self.visible_panel_mut() {
                panel.scroll_by(dir * step);
                return;
            }
        }
        self.scroll_history(-dir * step);
    }

    fn click(&mut self, x: usize, y: usize) {
        if self.input_disabled() {
            return;
        }
        let Some(a) = self.input_area else { return };
        if y < a.row || y >= a.row + a.height || x < a.col {
            return;
        }
        self.editor.apply(
            Edit::Click {
                row: y - a.row + a.top,
                col: x - a.col,
            },
            a.cols,
        );
        if !self.searching() && !self.answering() {
            self.update_token_picker();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyModifiers, MouseEvent};

    fn event(kind: MouseEventKind, column: u16, row: u16) -> MouseEvent {
        MouseEvent {
            kind,
            column,
            row,
            modifiers: KeyModifiers::NONE,
        }
    }

    #[test]
    fn hover_tracks_the_link_under_the_pointer() {
        use crossterm::event::{KeyCode, KeyEvent};
        let mut app = crate::ui::app::test_app("hover");
        let (cfg, note, other, marker) = (
            app.cwd.join("tuse.lua"),
            app.cwd.join("note.txt"),
            app.cwd.join("other.txt"),
            app.cwd.join("opened.txt"),
        );
        std::fs::write(&note, "hi").unwrap();
        std::fs::write(&other, "hi2").unwrap();
        std::fs::write(&cfg, "local t=require('tuse')\nreturn { open={ txt=function(p) t.write('opened.txt', p) end } }").unwrap();
        app.run_command("reload");

        let mut r = crate::frame::draw::Renderer::default();
        let draw =
            |r: &mut crate::frame::draw::Renderer, app: &mut App, w: u16, h: u16| -> String {
                let mut out = Vec::new();
                r.draw(app, &mut out, w, h).unwrap();
                String::from_utf8_lossy(&out).into_owned()
            };
        app.chat.push(crate::kit::entry::Entry::Assistant(
            "note.txt other.txt".into(),
        ));
        draw(&mut r, &mut app, 60, 12);
        let (la, lb) = (app.links[0].clone(), app.links[1].clone());
        assert!(app.mouse(event(MouseEventKind::Moved, la.start as u16, la.row as u16)));
        assert!(!app.mouse(event(
            MouseEventKind::Moved,
            (la.start + 1) as u16,
            la.row as u16
        )));
        assert_eq!(app.hover, Some(la.id));

        app.mouse(event(
            MouseEventKind::Down(MouseButton::Left),
            lb.start as u16,
            lb.row as u16,
        ));
        assert_eq!(
            (app.hover, app.pressed_link),
            (None, Some(((lb.row, lb.start), lb.id)))
        );
        let press_b = draw(&mut r, &mut app, 60, 12);
        assert!(press_b.contains("\x1b[38;2;222;222;222m"));
        app.key(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE), 10);
        app.mouse(event(
            MouseEventKind::Up(MouseButton::Left),
            lb.start as u16,
            lb.row as u16,
        ));
        assert_eq!((app.hover, app.pressed_link), (None, None));
        app.editor.clear();

        app.mouse(event(
            MouseEventKind::Down(MouseButton::Left),
            la.start as u16,
            la.row as u16,
        ));
        app.mouse(event(
            MouseEventKind::Up(MouseButton::Left),
            la.start as u16,
            la.row as u16,
        ));
        assert!(draw(&mut r, &mut app, 60, 12).contains("\x1b[38;2;189;222;171m"));
        let end = std::time::Instant::now() + std::time::Duration::from_secs(3);
        while !marker.exists() && std::time::Instant::now() < end {
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        assert!(marker.exists());
        std::fs::remove_file(&marker).unwrap();

        app.mouse(event(
            MouseEventKind::Down(MouseButton::Left),
            la.start as u16,
            la.row as u16,
        ));
        app.mouse(event(
            MouseEventKind::Drag(MouseButton::Left),
            (la.start + 3) as u16,
            la.row as u16,
        ));
        app.mouse(event(
            MouseEventKind::Drag(MouseButton::Left),
            la.start as u16,
            la.row as u16,
        ));
        app.mouse(event(
            MouseEventKind::Up(MouseButton::Left),
            la.start as u16,
            la.row as u16,
        ));
        app.mouse(event(
            MouseEventKind::Down(MouseButton::Left),
            la.start as u16,
            la.row as u16,
        ));
        if let Some(crate::kit::entry::Entry::Assistant(s)) = app.chat.get_mut(app.chat.len() - 1) {
            *s = "other.txt\n\nshifted note.txt".into();
        }
        draw(&mut r, &mut app, 60, 12);
        assert_eq!(app.pressed_link, None);
        app.mouse(event(
            MouseEventKind::Up(MouseButton::Left),
            la.start as u16,
            la.row as u16,
        ));
        std::thread::sleep(std::time::Duration::from_millis(60));
        assert!(!marker.exists());

        if let Some(crate::kit::entry::Entry::Assistant(s)) = app.chat.get_mut(app.chat.len() - 1) {
            *s = "a note.txt\n\nshifted b note.txt".into();
        }
        draw(&mut r, &mut app, 60, 12);
        let top = app.links[0].clone();
        app.mouse(event(
            MouseEventKind::Moved,
            top.start as u16,
            top.row as u16,
        ));
        app.chat
            .push(crate::kit::entry::Entry::Notice("shift".into()));
        draw(&mut r, &mut app, 60, 12);
        assert_eq!(app.hover, None);

        std::fs::write(
            &cfg,
            "return { theme = { accent = 'none', fg = '#445566' } }",
        )
        .unwrap();
        app.session.reload().unwrap();
        app.handle_event(tuse_core::Event::Reloaded);
        let none_ansi = draw(&mut r, &mut app, 60, 12);
        let at = none_ansi.find("note.txt").unwrap();
        let before = &none_ansi[at.saturating_sub(24)..at];
        assert!(before.contains("\x1b[4m") && !before.contains("\x1b[38;2;68;85;102m"));
        let first = app.links[0].clone();
        app.mouse(event(
            MouseEventKind::Moved,
            first.start as u16,
            first.row as u16,
        ));
        assert!(draw(&mut r, &mut app, 60, 12).contains("\x1b[38;2;68;85;102m"));

        for cmd in ["config", "session"] {
            std::fs::write(
                &cfg,
                "return { theme = { accent = '#112233', fg = '#445566' } }",
            )
            .unwrap();
            app.run_command("reload");
            app.run_command(cmd);
            assert!(draw(&mut r, &mut app, 80, 20).contains("\x1b[38;2;17;34;51m"));
            std::fs::write(
                &cfg,
                "return { theme = { accent = '#778899', fg = '#445566' } }",
            )
            .unwrap();
            app.session.reload().unwrap();
            app.handle_event(tuse_core::Event::Reloaded);
            let out = draw(&mut r, &mut app, 80, 20);
            assert!(out.contains("\x1b[38;2;119;136;153m") && !out.contains("\x1b[38;2;17;34;51m"));
        }
        crate::ui::app::cleanup(app);
    }

    #[test]
    fn wheel_and_page_keys_scroll_history_while_a_question_is_open() {
        use crossterm::event::{KeyCode, KeyEvent};

        let mut app = crate::ui::app::test_app("mouse-question");
        app.notice("previous message");
        app.notice("latest message");
        app.open_question(1, "question\n\n".repeat(25));
        app.paste("draft answer");
        let mut renderer = crate::frame::draw::Renderer::default();
        let mut screen = Vec::new();
        renderer.draw(&mut app, &mut screen, 80, 20).unwrap();
        assert!(String::from_utf8_lossy(&screen).contains("latest message"));
        let border = app.overlay_rows.start as u16;
        assert!(border > 0 && !app.overlay_rows.is_empty());
        let step = app.palette.ui.scroll_step as usize;
        app.mouse(event(MouseEventKind::ScrollDown, 0, border));
        assert_eq!(app.question.as_ref().unwrap().panel.scroll, step);
        assert_eq!(app.scroll, 0);
        app.mouse(event(MouseEventKind::ScrollUp, 0, border - 1));
        assert_eq!(app.scroll, step);
        assert_eq!(app.question.as_ref().unwrap().panel.scroll, step);
        renderer
            .draw(&mut app, &mut std::io::sink(), 80, 20)
            .unwrap();
        assert!(app.scroll > 0);
        app.key(KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE), 18);
        assert_eq!(app.scroll, 0);
        app.key(KeyEvent::new(KeyCode::PageUp, KeyModifiers::NONE), 18);
        assert_eq!(app.scroll, 18);
        app.key(KeyEvent::new(KeyCode::PageUp, KeyModifiers::SHIFT), 18);
        assert_eq!(app.question.as_ref().unwrap().panel.scroll, 0);
        assert_eq!(app.scroll, 18);
        assert_eq!(app.editor.text(), "draft answer");
        crate::ui::app::cleanup(app);
    }

    #[test]
    fn the_wheel_scrolls_the_chat_and_a_click_moves_the_cursor() {
        let mut app = crate::ui::app::test_app("mouse");
        let step = app.palette.ui.scroll_step as usize;
        app.mouse(event(MouseEventKind::ScrollUp, 0, 0));
        assert_eq!(app.scroll, step);
        app.mouse(event(MouseEventKind::ScrollDown, 0, 0));
        assert_eq!(app.scroll, 0);
        app.paste("hello");
        app.input_area = Some(crate::ui::app::InputArea {
            row: 20,
            col: 2,
            height: 1,
            top: 0,
            cols: 40,
        });
        app.mouse(event(MouseEventKind::Down(MouseButton::Left), 4, 20));
        assert_eq!(app.editor.cursor(), 2);
        app.mouse(event(MouseEventKind::Down(MouseButton::Left), 4, 3));
        assert_eq!(app.editor.cursor(), 2);
        crate::ui::app::cleanup(app);
    }
}
