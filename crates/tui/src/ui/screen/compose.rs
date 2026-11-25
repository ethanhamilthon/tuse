use crate::kit::style::Style;
use crate::kit::text::Line;
use crate::ui::app::App;
pub(crate) struct Frame {
    pub screen: Vec<Line>,
    pub pad: usize,
    pub base: Style,
    pub cursor: Option<(u16, u16)>,
}

impl App {
    pub(crate) fn reset_for_resize(&mut self) {
        self.selection = Default::default();
        self.clear_link_hover();
    }

    pub(crate) fn palette_gen(&self) -> u64 {
        self.palette_gen
    }

    pub(crate) fn compose(&mut self, w: usize, h: usize) -> Frame {
        let p = &self.palette;
        let ui = &p.ui;
        let pad = ui.pad_x as usize;
        let inner = w.saturating_sub(pad * 2).max(1);
        let left = crate::kit::text::width(crate::kit::style::INPUT_PREFIX);
        let boxed = ui.input_style == tuse_config::InputStyle::Boxed && h >= 5 && inner >= left + 3;
        let editor_w = inner - if boxed { 2 } else { 0 };
        let input_col = pad + left + usize::from(boxed);
        let text_w = editor_w.saturating_sub(left).max(1);
        self.input_cols = text_w;
        let mut view = self.editor.view(text_w);
        let (crow, ccol) = view.cursor;
        let head = usize::from(h > 1);
        let foot = usize::from(h > 2);
        let editor_room = if boxed {
            2
        } else {
            h.saturating_sub(head + foot).max(1)
        };
        let editor_rows = if boxed { 2 } else { view.rows.len() };
        let input_h = tuse_config::limit(ui.input_max_height, editor_rows)
            .max(1)
            .min(editor_room);
        let input_top = crow
            .saturating_sub(input_h.saturating_sub(1))
            .min(view.rows.len().saturating_sub(input_h));
        let fixed = if boxed { 5 } else { input_h + head + foot };
        let chat_h = h.saturating_sub(fixed);
        let mut screen: Vec<Line> = Vec::with_capacity(h);

        let cap = (chat_h - usize::from(chat_h > 2)).min(p.ui.list_max_height);
        let mut overlay = if let Some(pk) = &mut self.picker {
            let rows = cap.saturating_sub(1).max(1);
            let lines = crate::ui::screen::list::options(pk, rows, inner, p);
            self.list_rows = rows;
            crate::kit::rich::render::framed(lines, inner, p)
        } else if let Some(panel) = self
            .question
            .as_mut()
            .map(|q| &mut q.panel)
            .or(self.panel.as_mut())
        {
            panel.lines(inner, cap, p)
        } else {
            vec![]
        };
        overlay.truncate(cap);
        let history_h = chat_h - overlay.len();
        self.overlay_rows = history_h..chat_h;
        if history_h > 0 {
            if self.scroll > 0 {
                let total = self.chat.total(inner, p);
                if let Some((w, prev)) = self.chat_anchor
                    && w == inner
                    && total > prev
                {
                    self.scroll += total - prev;
                }
                self.chat_anchor = Some((inner, total));
            } else {
                self.chat_anchor = None;
            }
            let (view, clamped) = self.chat.window(inner, p, self.scroll, history_h);
            self.scroll = clamped;
            screen.resize_with(history_h - view.len(), Vec::new);
            screen.extend(view);
        }
        screen.extend(overlay);
        view.rows
            .resize_with(view.rows.len().max(input_h), String::new);
        let shown = &view.rows[input_top..input_top + input_h];
        if boxed {
            screen.extend(self.boxed_input(&view, editor_w, shown, input_top));
        } else {
            screen.extend(
                self.input_block(&view, editor_w, shown, input_top)
                    .into_iter()
                    .skip(1 - head)
                    .take(head + input_h + foot),
            );
        }
        screen.truncate(h);
        self.links = crate::kit::links::scan(&screen, pad);
        if let Some(id) = self.hover
            && self.pointer.is_none_or(|pt| self.link_at(pt) != Some(id))
        {
            self.hover = None;
        }
        if let Some((pt, id)) = self.pressed_link
            && self.link_at(pt) != Some(id)
        {
            self.pressed_link = None;
        }
        crate::kit::links::highlight(
            &mut screen,
            self.pressed_link.map(|(_, id)| id).or(self.hover),
            p.fg,
        );
        self.selection
            .render(&mut screen, pad, Style::fg(p.bg).on(p.accent));

        let first = if boxed {
            h - 4
        } else {
            h.saturating_sub(input_h + foot)
        };
        self.input_area = Some(crate::ui::app::InputArea {
            row: first,
            col: input_col,
            height: input_h,
            top: input_top,
            cols: text_w,
        });
        let cursor = (!self.input_disabled()).then(|| {
            let cy = (first + crow - input_top).min(h.saturating_sub(1));
            ((input_col + ccol) as u16, cy as u16)
        });
        let p = &self.palette;
        Frame {
            screen,
            pad,
            base: Style::fg(p.fg).on(p.bg),
            cursor,
        }
    }
}
#[cfg(test)]
mod tests {
    use crate::kit::text::line_width;
    use crate::ui::app::{cleanup, test_app};
    use tuse_config::InputStyle;

    #[test]
    fn boxed_layout_tracks_the_cursor_and_survives_resize() {
        let mut app = test_app("boxed-layout");
        app.palette.ui.input_style = InputStyle::Boxed;
        app.editor.insert("one\ntwo\nthree\nfour");
        app.display_flash("Waiting");
        let frame = app.compose(80, 10);
        let area = app.input_area.unwrap();
        assert_eq!((area.row, area.col, area.height, area.top), (6, 3, 2, 2));
        assert_eq!(frame.cursor, Some((7, 7)));
        assert!(frame.screen[5..].iter().all(|row| line_width(row) == 80));
        assert!(frame.screen[5][0].text.starts_with('╭'));
        assert_eq!(frame.screen[8][1].text, "Waiting");
        assert!(frame.screen[9][0].text.starts_with('╰'));
        assert!(
            !app.compose(80, 4)
                .screen
                .iter()
                .flatten()
                .any(|s| s.text.starts_with('╭'))
        );
        assert!(app.compose(80, 10).screen[5][0].text.starts_with('╭'));
        assert_eq!(app.editor.text(), "one\ntwo\nthree\nfour");
        cleanup(app);
    }
}
