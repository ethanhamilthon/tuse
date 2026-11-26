use crate::kit::editor::Edit;
use crate::kit::selection::{Gesture, MessageSelection};
use crate::ui::app::App;
use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};

impl App {
    pub fn mouse(&mut self, m: MouseEvent) -> bool {
        let point = (m.row as usize, m.column as usize);
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
                if self.input_area.is_some_and(|a| a.contains(point)) && !self.input_disabled() {
                    self.selection.gesture = Some(Gesture::Input(self.editor.cursor()));
                } else if let Some(hit) = self.chat_hits.get(point.0).and_then(Option::as_ref)
                    && let Some(col) = point.1.checked_sub(self.chat_pad)
                    && hit.contains(col)
                {
                    self.selection.gesture = Some(Gesture::Chat {
                        id: hit.id,
                        anchor: hit.at(col),
                    });
                }
                true
            }
            MouseEventKind::Drag(MouseButton::Left) | MouseEventKind::Up(MouseButton::Left) => {
                match self.selection.gesture {
                    Some(Gesture::Input(anchor)) => {
                        if let Some(a) = self.input_area.filter(|_| !self.input_disabled()) {
                            let (row, col) = if point.0 < a.row {
                                (a.top, 0)
                            } else if point.0 >= a.row + a.height {
                                (a.top + a.height - 1, a.cols)
                            } else {
                                (
                                    a.top + point.0 - a.row,
                                    point.1.saturating_sub(a.col).min(a.cols),
                                )
                            };
                            self.editor.apply(Edit::Drag { row, col, anchor }, a.cols);
                            if !self.searching() && !self.answering() {
                                self.update_token_picker();
                            }
                        }
                    }
                    Some(Gesture::Chat { id, anchor }) => {
                        let col = point.1.saturating_sub(self.chat_pad);
                        let rows = || {
                            self.chat_hits.iter().enumerate().filter_map(|(y, hit)| {
                                hit.as_ref()
                                    .filter(|hit| hit.id == id && hit.is_text())
                                    .map(|hit| (y, hit))
                            })
                        };
                        let end = rows()
                            .find(|(y, _)| *y >= point.0)
                            .map(|(y, hit)| hit.at(if y == point.0 { col } else { 0 }))
                            .or_else(|| rows().next_back().map(|(_, hit)| hit.at(hit.end())));
                        self.selection.message = end
                            .filter(|end| *end != anchor)
                            .map(|end| MessageSelection { id, anchor, end });
                    }
                    None => {}
                }
                if matches!(m.kind, MouseEventKind::Drag(_)) {
                    self.clear_link_hover();
                } else {
                    self.selection.gesture = None;
                    if let Some((pt, id)) = self.pressed_link.take()
                        && pt == point
                        && self.link_at(point) == Some(id)
                        && let Some(target) = crate::kit::links::target(id)
                    {
                        match target {
                            crate::kit::links::LinkTarget::Url(u) => self.session.open_url(&u),
                            crate::kit::links::LinkTarget::Path(p) => self.session.open_path(&p),
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
        if y < a.row || y >= a.row + a.height || x < a.col || (a.boxed && x >= a.col + a.cols) {
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
        std::fs::write(&cfg, "local t=require('tuse')\nreturn { settings={ open={ txt=function(p) t.write('opened.txt', p) end } } }").unwrap();
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
        assert_eq!(app.question.as_ref().unwrap().panel.scroll(), step);
        assert_eq!(app.scroll, 0);
        app.mouse(event(MouseEventKind::ScrollUp, 0, border - 1));
        assert_eq!(app.scroll, step);
        assert_eq!(app.question.as_ref().unwrap().panel.scroll(), step);
        renderer
            .draw(&mut app, &mut std::io::sink(), 80, 20)
            .unwrap();
        assert!(app.scroll > 0);
        app.key(KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE), 18);
        assert_eq!(app.scroll, 0);
        app.key(KeyEvent::new(KeyCode::PageUp, KeyModifiers::NONE), 18);
        assert_eq!(app.scroll, 18);
        app.key(KeyEvent::new(KeyCode::PageUp, KeyModifiers::SHIFT), 18);
        assert_eq!(app.question.as_ref().unwrap().panel.scroll(), 0);
        assert_eq!(app.scroll, 18);
        assert_eq!(app.editor.text(), "draft answer");
        crate::ui::app::cleanup(app);
    }

    #[test]
    fn boxed_mouse_selection_aligns_with_wrapped_text_not_padding() {
        let mut app = crate::ui::app::test_app("boxed-mouse");
        app.palette.ui.input_style = tuse_config::InputStyle::Boxed;
        app.palette.ui.pad_x = 1;
        app.editor.insert("abcd界efghi");
        let frame = app.compose(12, 8);
        let area = app.input_area.unwrap();
        assert_eq!((area.height, area.col, area.cols), (2, 3, 6));
        let y = (area.row + 1) as u16;
        assert_eq!(frame.cursor, Some((8, y)));
        app.mouse(event(MouseEventKind::Down(MouseButton::Left), 5, y));
        assert_eq!(app.editor.cursor(), 7);
        app.mouse(event(MouseEventKind::Up(MouseButton::Left), 7, y));
        assert_eq!(app.editor.selected_text().as_deref(), Some("gh"));
        for gutter in [2, 9] {
            app.mouse(event(MouseEventKind::Down(MouseButton::Left), gutter, y));
            app.mouse(event(MouseEventKind::Up(MouseButton::Left), gutter, y));
            assert_eq!(app.editor.cursor(), 9);
        }
        crate::ui::app::cleanup(app);
    }

    fn drag(app: &mut App, from: (usize, usize), to: (usize, usize)) {
        app.mouse(event(
            MouseEventKind::Down(MouseButton::Left),
            from.1 as u16,
            from.0 as u16,
        ));
        app.mouse(event(
            MouseEventKind::Drag(MouseButton::Left),
            to.1 as u16,
            to.0 as u16,
        ));
        app.mouse(event(
            MouseEventKind::Up(MouseButton::Left),
            to.1 as u16,
            to.0 as u16,
        ));
    }

    fn clipboard(app: &mut App, mods: KeyModifiers) -> String {
        use crossterm::event::{KeyCode, KeyEvent};
        app.key(KeyEvent::new(KeyCode::Char('c'), mods), 10);
        crate::kit::clipboard::TEST_TEXT.with(|s| s.borrow().clone())
    }

    fn chat_bounds(app: &App) -> ((usize, usize), (usize, usize)) {
        let points = || {
            app.chat_hits.iter().enumerate().filter_map(|(y, hit)| {
                let hit = hit.as_ref().filter(|hit| hit.is_text())?;
                Some((
                    (y, app.chat_pad + hit.start()),
                    (y, app.chat_pad + hit.end()),
                ))
            })
        };
        (points().next().unwrap().0, points().next_back().unwrap().1)
    }

    #[test]
    fn drags_stay_in_their_starting_message_or_input() {
        use crate::kit::entry::Entry;
        use crossterm::event::{KeyCode, KeyEvent};
        let mut app = crate::ui::app::test_app("scoped-selection");
        app.palette.ui.pad_x = 2;
        app.palette.ui.reasoning_line = -1;
        app.chat.push(Entry::User {
            text: "user text".into(),
            images: 0,
        });
        app.chat.push(Entry::Assistant("agent text".into()));
        app.chat.push(Entry::Reasoning("thought".into()));
        app.chat.push(Entry::Tool {
            id: "id".into(),
            input: "return 1".into(),
            output: Some("1".into()),
            code: Some(0),
            diffs: vec![],
        });
        app.chat.push(Entry::Notice("event".into()));
        app.chat.push(Entry::UiResult {
            is_picker: true,
            prompt: "picker".into(),
            value: "result".into(),
        });
        app.chat
            .set_markdown("card".into(), "show".into(), true, false);
        app.editor.insert("draft\n  first line\nlast line\n");
        app.palette.ui.input_max_height = 2;
        app.panel = Some(crate::ui::panel::Panel::new("overlay"));
        let frame = app.compose(44, 25);
        let (user, agent_end) = chat_bounds(&app);
        let agent = (agent_end.0, user.1);
        let area = app.input_area.unwrap();
        for row in agent.0 + 1..area.row {
            drag(&mut app, (row, agent.1), user);
            assert!(app.selection.message.is_none());
        }
        let overlay = app.overlay_rows.start;
        assert!(!app.overlay_rows.is_empty());
        drag(&mut app, (overlay, agent.1), user);
        assert!(app.selection.message.is_none());
        drag(&mut app, agent_end, user);
        assert_eq!(clipboard(&mut app, KeyModifiers::CONTROL), "agent text");
        drag(&mut app, agent, (area.row, area.col + 2));
        app.compose(44, 25);
        assert!(app.editor.selected_text().is_none());
        assert_eq!(clipboard(&mut app, KeyModifiers::SUPER), "agent text");
        drag(&mut app, user, agent_end);
        assert_eq!(clipboard(&mut app, KeyModifiers::CONTROL), "user text");
        for primary in [KeyModifiers::CONTROL, KeyModifiers::SUPER] {
            drag(&mut app, agent, agent_end);
            app.key(KeyEvent::new(KeyCode::Char('a'), primary), 10);
            assert!(app.selection.message.is_none());
            assert_eq!(
                clipboard(&mut app, primary),
                "draft\n  first line\nlast line\n"
            );
        }
        app.editor.clear();
        app.editor.insert("hello draft");
        app.compose(44, frame.screen.len());
        let area = app.input_area.unwrap();
        drag(&mut app, (area.row, area.col + 5), user);
        assert_eq!(clipboard(&mut app, KeyModifiers::CONTROL), "hello");
        crate::ui::app::cleanup(app);
    }

    #[test]
    fn clipboard_joins_visual_wraps_but_preserves_paragraphs_and_code() {
        use crate::kit::entry::Entry;
        let mut app = crate::ui::app::test_app("logical-copy");
        let paragraph = "Hello 世界 and e\u{301} extraordinaryword with spaces.";
        let code = "fn f() {\n    one();\n\n    two();\n}";
        let message = format!(
            "**Hello** 世界 and e\u{301} [extraordinaryword](https://example.org) with spaces.\n\n```rust\n{code}\n```"
        );
        app.chat.push(Entry::Assistant(message));
        for width in [15, 27, 60] {
            app.compose(width, 32);
            let (first, last) = chat_bounds(&app);
            drag(&mut app, first, last);
            assert_eq!(
                clipboard(&mut app, KeyModifiers::CONTROL),
                format!("{paragraph}\n\n{code}")
            );
        }
        app.chat.remove(app.chat.len() - 1);
        app.chat.push(Entry::Assistant("| First | Second |\n| :---- | -----: |\n| long words in one cell | more words in another |".into()));
        for width in [26, 60] {
            app.compose(width, 20);
            let (first, last) = chat_bounds(&app);
            drag(&mut app, first, last);
            assert_eq!(
                clipboard(&mut app, KeyModifiers::CONTROL),
                "First | Second\nlong words in one cell | more words in another"
            );
        }
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
            boxed: false,
        });
        app.mouse(event(MouseEventKind::Down(MouseButton::Left), 4, 20));
        assert_eq!(app.editor.cursor(), 2);
        app.mouse(event(MouseEventKind::Down(MouseButton::Left), 4, 3));
        assert_eq!(app.editor.cursor(), 2);
        crate::ui::app::cleanup(app);
    }
}
