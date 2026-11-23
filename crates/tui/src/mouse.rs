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
                self.click(point.1, point.0);
                self.selection.drag = Some((point, point));
                if inside && !self.input_disabled() {
                    self.selection.input = Some(self.editor.cursor);
                }
                true
            }
            MouseEventKind::Drag(MouseButton::Left) | MouseEventKind::Up(MouseButton::Left) => {
                if let Some((a, _)) = self.selection.drag {
                    if inside && let Some(index) = self.selection.input {
                        self.click(point.1, point.0);
                        self.editor.anchor = Some(index);
                        self.selection.drag = Some((a, a));
                    } else {
                        self.editor.clear_selection();
                        self.selection.drag = Some((a, point));
                    }
                    if matches!(m.kind, MouseEventKind::Up(_)) {
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
