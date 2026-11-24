use crate::app::App;
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
                let at = self.link_at(point);
                std::mem::replace(&mut self.hover, at) != at
            }
            MouseEventKind::Down(MouseButton::Left) => {
                self.editor.clear_selection();
                self.selection = Default::default();
                self.pressed_link = self.link_at(point);
                self.click(point.1, point.0);
                self.selection.drag = Some((point, point));
                if inside && !self.input_disabled() {
                    self.selection.input = Some(self.editor.cursor);
                }
                true
            }
            MouseEventKind::Drag(MouseButton::Left) | MouseEventKind::Up(MouseButton::Left) => {
                if let Some((a, _)) = self.selection.drag {
                    if a != point {
                        self.clear_link_hover();
                    }
                    if inside && let Some(index) = self.selection.input {
                        self.click(point.1, point.0);
                        self.editor.anchor = Some(index);
                        self.selection.drag = Some((a, a));
                    } else {
                        self.editor.clear_selection();
                        self.selection.drag = Some((a, point));
                    }
                    if matches!(m.kind, MouseEventKind::Up(_)) {
                        self.pressed_link = None;
                        self.selection.input = None;
                        if self.selection.drag == Some((a, a)) {
                            self.selection.drag = None;
                            if a == point
                                && let Some(t) = self.link_at(point).and_then(crate::links::target)
                            {
                                match t {
                                    crate::links::LinkTarget::Url(u) => self.session.open_url(&u),
                                    crate::links::LinkTarget::Path(p) => self.session.open_path(&p),
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

    fn link_at(&self, (row, col): (usize, usize)) -> Option<u32> {
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
        self.editor.click(y - a.row + a.top, x - a.col, a.cols);
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
        let mut app = crate::app::test_app("hover");
        let id = crate::links::intern_url("https://hover.example");
        app.links = vec![crate::links::LinkBox {
            row: 2,
            start: 4,
            end: 9,
            id,
        }];
        assert!(app.mouse(event(MouseEventKind::Moved, 4, 2)));
        assert_eq!(app.hover, Some(id));
        assert!(
            !app.mouse(event(MouseEventKind::Moved, 8, 2)),
            "same link: no redraw"
        );
        assert!(app.mouse(event(MouseEventKind::Moved, 9, 2)));
        assert_eq!(app.hover, None);

        app.mouse(event(MouseEventKind::Down(MouseButton::Left), 4, 2));
        assert_eq!(app.pressed_link, Some(id));
        assert_eq!(app.hover, None);
        app.mouse(event(MouseEventKind::Drag(MouseButton::Left), 6, 2));
        assert_eq!(app.pressed_link, None);
        assert_eq!(app.hover, None);
        assert_eq!(app.selection.drag, Some(((2, 4), (2, 6))));
        app.mouse(event(MouseEventKind::Up(MouseButton::Left), 6, 2));
        assert_eq!(app.pressed_link, None);

        app.mouse(event(MouseEventKind::Moved, 4, 2));
        assert_eq!(app.hover, Some(id));
        app.mouse(event(MouseEventKind::ScrollUp, 4, 2));
        assert_eq!(app.hover, None);

        let cfg = app.cwd.join("tuse.lua");
        let note = app.cwd.join("note.txt");
        let marker = app.cwd.join("opened.txt");
        std::fs::write(&note, "hi").unwrap();
        std::fs::write(
            &cfg,
            r#"local t = require("tuse")
return { open = { txt = function(p) t.write("opened.txt", p) end } }"#,
        )
        .unwrap();
        app.run_command("reload");

        let mut renderer = crate::draw::Renderer::default();
        app.chat
            .push(crate::chat::Entry::Assistant("see note.txt".into()));
        let mut out = Vec::new();
        renderer.draw(&mut app, &mut out, 60, 12).unwrap();
        let link = app.links[0].clone();
        assert_eq!(app.hover, None);
        app.mouse(event(
            MouseEventKind::Down(MouseButton::Left),
            link.start as u16,
            link.row as u16,
        ));
        assert_eq!(app.pressed_link, Some(link.id));
        out.clear();
        renderer.draw(&mut app, &mut out, 60, 12).unwrap();
        let pressed_ansi = String::from_utf8_lossy(&out);
        assert!(pressed_ansi.contains("\x1b[38;2;222;222;222m"));
        app.mouse(event(
            MouseEventKind::Up(MouseButton::Left),
            link.start as u16,
            link.row as u16,
        ));
        assert_eq!(app.pressed_link, None);
        assert_eq!(app.hover, None);
        out.clear();
        renderer.draw(&mut app, &mut out, 60, 12).unwrap();
        let resting_ansi = String::from_utf8_lossy(&out);
        assert!(resting_ansi.contains("\x1b[38;2;189;222;171m"));
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        while !marker.exists() && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        assert!(marker.exists());

        app.mouse(event(
            MouseEventKind::Moved,
            link.start as u16,
            link.row as u16,
        ));
        assert_eq!(app.hover, Some(link.id));
        renderer
            .draw(&mut app, &mut std::io::sink(), 62, 12)
            .unwrap();
        assert_eq!(app.hover, None);

        std::fs::write(
            &cfg,
            "return { theme = { accent = '#112233', fg = '#445566' } }",
        )
        .unwrap();
        app.session.reload().unwrap();
        app.handle_event(tuse_core::Event::Reloaded);
        out.clear();
        renderer.draw(&mut app, &mut out, 62, 12).unwrap();
        let reloaded_ansi = String::from_utf8_lossy(&out);
        assert!(reloaded_ansi.contains("\x1b[38;2;17;34;51m"));
        let link = app.links[0].clone();
        app.mouse(event(
            MouseEventKind::Moved,
            link.start as u16,
            link.row as u16,
        ));
        out.clear();
        renderer.draw(&mut app, &mut out, 62, 12).unwrap();
        let hovered_ansi = String::from_utf8_lossy(&out);
        assert!(hovered_ansi.contains("\x1b[38;2;68;85;102m"));

        crate::app::cleanup(app);
    }

    #[test]
    fn wheel_and_page_keys_scroll_history_while_a_question_is_open() {
        use crossterm::event::{KeyCode, KeyEvent};

        let mut app = crate::app::test_app("mouse-question");
        app.notice("previous message");
        app.notice("latest message");
        app.open_question(1, "question\n\n".repeat(25));
        app.paste("draft answer");
        let mut renderer = crate::draw::Renderer::default();
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
        crate::app::cleanup(app);
    }

    #[test]
    fn the_wheel_scrolls_the_chat_and_a_click_moves_the_cursor() {
        let mut app = crate::app::test_app("mouse");
        let step = app.palette.ui.scroll_step as usize;
        app.mouse(event(MouseEventKind::ScrollUp, 0, 0));
        assert_eq!(app.scroll, step);
        app.mouse(event(MouseEventKind::ScrollDown, 0, 0));
        assert_eq!(app.scroll, 0);
        app.paste("hello");
        app.input_area = Some(crate::app::InputArea {
            row: 20,
            col: 2,
            height: 1,
            top: 0,
            cols: 40,
        });
        app.mouse(event(MouseEventKind::Down(MouseButton::Left), 4, 20));
        assert_eq!(app.editor.cursor, 2);
        app.mouse(event(MouseEventKind::Down(MouseButton::Left), 4, 3));
        assert_eq!(app.editor.cursor, 2);
        crate::app::cleanup(app);
    }
}
