use crate::app::App;
use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};

impl App {
    /// The wheel scrolls the chat, the open panel or moves the selection of an open list;
    /// a left click in the input puts the cursor there.
    pub fn mouse(&mut self, m: MouseEvent) -> bool {
        let point = (m.row as usize, m.column as usize);
        let inside = self.input_area.is_some_and(|a| {
            point.0 >= a.row
                && point.0 < a.row + a.height
                && point.1 >= a.col
                && point.1 < a.col + a.cols
        }) && self.picker.as_ref().is_none_or(|p| p.kind.is_token());
        match m.kind {
            MouseEventKind::ScrollUp => {
                self.wheel(-1);
                true
            }
            MouseEventKind::ScrollDown => {
                self.wheel(1);
                true
            }
            MouseEventKind::Down(MouseButton::Left) => {
                self.editor.clear_selection();
                self.selection = Default::default();
                self.click(point.1, point.0);
                self.selection.drag = Some((point, point));
                if inside {
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
                        }
                    }
                }
                true
            }
            _ => false,
        }
    }

    /// `dir` is `-1` for up, `1` for down.
    fn wheel(&mut self, dir: isize) {
        self.selection = Default::default();
        let step = self.palette.ui.scroll_step as isize;
        if let Some(p) = &mut self.picker {
            let n = p.shown.len() as isize;
            if n > 0 {
                p.selected = (p.selected as isize + dir).clamp(0, n - 1) as usize;
                p.follow(self.list_rows);
            }
        } else if let Some(panel) = &mut self.panel {
            panel.scroll_by(dir * step);
        } else if dir < 0 {
            self.scroll += step as usize;
        } else {
            self.scroll = self.scroll.saturating_sub(step as usize);
        }
    }

    fn click(&mut self, x: usize, y: usize) {
        if self.picker.as_ref().is_some_and(|p| !p.kind.is_token()) {
            return;
        }
        let Some(a) = self.input_area else { return };
        if y < a.row || y >= a.row + a.height || x < a.col {
            return;
        }
        self.editor.click(y - a.row + a.top, x - a.col, a.cols);
        self.update_token_picker();
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
    fn the_wheel_scrolls_the_chat_and_a_click_moves_the_cursor() {
        let mut app = crate::app::test_app("mouse", true);
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

    #[test]
    fn an_ask_sends_the_typed_text_else_the_selected_option() {
        let mut app = crate::app::test_app("ask", false);
        let s = app.session.clone();
        app.editor.insert("draft");
        let mut ask = |options: &[&str], typed: &str| {
            let (_, rx) = s
                .ask("Which?", options.iter().map(|o| o.to_string()).collect())
                .unwrap();
            while app.picker.is_none() {
                let e = app
                    .events
                    .recv_timeout(std::time::Duration::from_secs(5))
                    .unwrap();
                app.handle_event(e);
            }
            if !typed.is_empty() {
                app.picker.as_mut().unwrap().selected = options.len();
                app.choose();
                assert!(app.editor.is_empty());
            }
            for c in typed.chars() {
                app.key(
                    crossterm::event::KeyEvent::new(
                        crossterm::event::KeyCode::Char(c),
                        KeyModifiers::NONE,
                    ),
                    0,
                );
            }
            if typed.is_empty() {
                app.choose();
            } else {
                app.key(
                    crossterm::event::KeyEvent::new(
                        crossterm::event::KeyCode::Enter,
                        KeyModifiers::NONE,
                    ),
                    0,
                );
            }
            assert_eq!(app.editor.text(), "draft");
            rx.recv_timeout(std::time::Duration::from_secs(5)).unwrap()
        };
        assert_eq!(ask(&["alpha", "beta"], "al"), "al");
        assert_eq!(ask(&["alpha", "beta"], ""), "alpha");
        assert_eq!(ask(&[], "my own words"), "my own words");
        for cancel in [false, true] {
            app.handle_event(tuse_core::Event::Ask {
                id: "pending".into(),
                question: "Which?".into(),
                options: vec![],
            });
            app.choose();
            app.paste("custom");
            if cancel {
                app.key(
                    crossterm::event::KeyEvent::new(
                        crossterm::event::KeyCode::Esc,
                        KeyModifiers::NONE,
                    ),
                    0,
                );
            } else {
                app.handle_event(tuse_core::Event::AskDone {
                    id: "pending".into(),
                });
            }
            assert!(app.ask_input.is_none());
            assert_eq!(app.editor.text(), "draft");
        }
        crate::app::cleanup(app);
    }
}
