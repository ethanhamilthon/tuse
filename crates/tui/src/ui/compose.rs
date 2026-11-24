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
        let text_w = inner.saturating_sub(left).max(1);
        self.input_cols = text_w;
        let view = self.editor.view(text_w);
        let (crow, ccol) = view.cursor;
        let rows = view.rows.clone();
        let head = usize::from(h > 1);
        let foot = usize::from(h > 2);
        let editor_room = h.saturating_sub(head + foot).max(1);
        let input_h = tuse_config::limit(ui.input_max_height, rows.len())
            .max(1)
            .min(editor_room);
        let input_top = crow
            .saturating_sub(input_h.saturating_sub(1))
            .min(rows.len().saturating_sub(input_h));
        let fixed = input_h + head + foot;
        let chat_h = h.saturating_sub(fixed);
        let mut screen: Vec<Line> = Vec::with_capacity(h);

        let cap = (chat_h - usize::from(chat_h > 2)).min(p.ui.list_max_height);
        let mut overlay = if self.picker.is_some() {
            {
                let p = &self.palette;
                match &mut self.picker {
                    Some(pk) => {
                        let rows = cap.saturating_sub(1).max(1);
                        let lines = crate::ui::list::options(pk, rows, inner, p);
                        self.list_rows = rows;
                        crate::kit::rich::render::framed(lines, inner, p)
                    }
                    None => vec![],
                }
            }
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
        let p = &self.palette;
        if history_h > 0 {
            let (view, clamped) = self.chat.window(inner, p, self.scroll, history_h);
            self.scroll = clamped;
            screen.resize_with(history_h - view.len(), Vec::new);
            screen.extend(view);
        }
        screen.extend(overlay);
        let mut rows = rows;
        rows.resize_with(rows.len().max(input_h), String::new);
        let shown = &rows[input_top..input_top + input_h];
        screen.extend(
            self.input_block(&view, inner, shown, input_top)
                .into_iter()
                .skip(1 - head)
                .take(head + input_h + foot),
        );
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

        let first = h.saturating_sub(input_h + foot);
        self.input_area = Some(crate::ui::app::InputArea {
            row: first,
            col: pad + left,
            height: input_h,
            top: input_top,
            cols: text_w,
        });
        let cursor = (!self.input_disabled()).then(|| {
            let cy = (first + crow - input_top).min(h.saturating_sub(1));
            ((pad + left + ccol) as u16, cy as u16)
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
