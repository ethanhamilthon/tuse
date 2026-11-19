use crate::app::App;
use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};

impl App {
    /// The wheel scrolls the chat, the open panel or moves the selection of an open list;
    /// a left click in the input puts the cursor there.
    pub fn mouse(&mut self, m: MouseEvent) {
        match m.kind {
            MouseEventKind::ScrollUp => self.wheel(-1),
            MouseEventKind::ScrollDown => self.wheel(1),
            MouseEventKind::Down(MouseButton::Left) => {
                self.click(m.column as usize, m.row as usize)
            }
            _ => {}
        }
    }

    /// `dir` is `-1` for up, `1` for down.
    fn wheel(&mut self, dir: isize) {
        let step = self.palette.t.scroll_step as isize;
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

    fn app() -> App {
        let dir = std::env::temp_dir().join(format!("tuse-mouse-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let s = tuse_core::Session::open(tuse_core::Options {
            cwd: dir.clone(),
            config: None,
            headless: true,
            resume: None,
        })
        .unwrap();
        App::new(s, dir, None)
    }

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
        let mut app = app();
        let step = app.palette.t.scroll_step as usize;
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
        let s = app.session.clone();
        s.close();
        std::fs::remove_dir_all(tuse_core::tuse_home().join(&s.id)).ok();
        std::fs::remove_dir_all(app.cwd.clone()).ok();
    }

    #[test]
    fn an_ask_sends_the_typed_text_else_the_selected_option() {
        let dir = std::env::temp_dir().join(format!("tuse-ask-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let s = tuse_core::Session::open(tuse_core::Options {
            cwd: dir.clone(),
            config: None,
            headless: false,
            resume: None,
        })
        .unwrap();
        let mut app = App::new(s.clone(), dir, None);
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
            // Typing is the answer, not a filter: the options stay.
            for c in typed.chars() {
                app.key(
                    crossterm::event::KeyEvent::new(
                        crossterm::event::KeyCode::Char(c),
                        KeyModifiers::NONE,
                    ),
                    0,
                );
            }
            assert_eq!(app.picker.as_ref().unwrap().shown.len(), options.len() + 1);
            app.choose();
            rx.recv_timeout(std::time::Duration::from_secs(5)).unwrap()
        };
        assert_eq!(ask(&["alpha", "beta"], "al"), "al");
        assert_eq!(ask(&["alpha", "beta"], ""), "alpha");
        assert_eq!(ask(&[], "my own words"), "my own words");
        s.close();
        std::fs::remove_dir_all(tuse_core::tuse_home().join(&s.id)).ok();
        std::fs::remove_dir_all(app.cwd.clone()).ok();
    }
}
