use crate::kit::selection::{HitRow, PICKER_BASE, is_overlay};
use crate::kit::style::Style;
use crate::kit::text::{CopyRow, CopySpan, Line, width};
use crate::ui::app::App;
use crate::ui::picker::Kind;
use crate::ui::screen::input_view::BOXED_INSET;
use tuse_config::InputStyle;
fn option_copy(line: &Line) -> Option<CopyRow> {
    let mark = line.first().map_or(0, |s| width(&s.text));
    let text: String = line.iter().skip(1).map(|s| s.text.as_str()).collect();
    let text = text.trim_end().to_string();
    (!text.is_empty()).then_some(CopyRow::Text(CopySpan {
        col: mark,
        text,
        soft: None,
    }))
}

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
        let pad = (ui.pad_x as usize).min(w.saturating_sub(1) / 2);
        let inner = w.saturating_sub(pad * 2).max(1);
        let inset = if ui.input_style == InputStyle::Boxed {
            BOXED_INSET
        } else {
            0
        };
        let min_width = if inset == 0 { 4 } else { 2 * inset + 2 };
        let (style, inset) = if h >= 5 && inner >= min_width {
            (ui.input_style, inset)
        } else {
            (InputStyle::Lines, 0)
        };
        let left = self.input_left(style);
        let editor_w = inner - 2 * inset;
        let input_col = pad + inset + left;
        let text_w = editor_w.saturating_sub(left).max(1);
        self.input_cols = text_w;
        let mut view = self.editor.view(text_w);
        let (crow, ccol) = view.cursor;
        let (lead, extra) = match style {
            InputStyle::Boxed | InputStyle::Fill => (1, 4),
            InputStyle::Bar => (0, 2),
            InputStyle::Lines => (usize::from(h > 1), usize::from(h > 1) + usize::from(h > 2)),
        };
        let gap = usize::from(h >= extra + 2);
        let editor_room = h.saturating_sub(extra + gap).max(1);
        let input_h = tuse_config::limit(ui.input_max_height, view.rows.len())
            .max(1)
            .min(editor_room);
        let input_top = crow
            .saturating_sub(input_h.saturating_sub(1))
            .min(view.rows.len().saturating_sub(input_h));
        let chat_h = h.saturating_sub(extra + gap + input_h);
        self.page_rows = chat_h;
        let mut screen: Vec<Line> = Vec::with_capacity(h);

        let cap = (chat_h - usize::from(chat_h > 2)).min(p.ui.list_max_height);
        let (mut overlay, mut overlay_hits) = if let Some(pk) = &mut self.picker {
            let mut rows = pk.title.as_deref().map_or_else(Vec::new, |title| {
                crate::kit::rich::markdown::rows(title, inner, p, false)
            });
            rows.truncate(cap.saturating_sub(1));
            let titled = rows.len();
            let count = (cap - titled).max(1);
            let options = crate::ui::screen::list::options(pk, count, inner, p);
            self.list_rows = count;
            let id = match pk.kind {
                Kind::LuaPicker(n) => Some(PICKER_BASE + n),
                _ => None,
            };
            let first = titled + pk.scroll;
            let mut hits: Vec<Option<HitRow>> = rows
                .iter()
                .enumerate()
                .map(|(index, r)| {
                    let copy = r.copy.clone();
                    id.zip(copy).map(|(id, copy)| HitRow { id, index, copy })
                })
                .collect();
            hits.extend(options.iter().enumerate().map(|(i, line)| {
                let copy = option_copy(line);
                id.zip(copy).map(|(id, copy)| HitRow {
                    id,
                    index: first + i,
                    copy,
                })
            }));
            let mut lines: Vec<Line> = rows.into_iter().map(|r| r.line).collect();
            lines.extend(options);
            (crate::kit::rich::render::framed(lines, inner, p), hits)
        } else if let Some(panel) = self
            .question
            .as_mut()
            .map(|q| &mut q.panel)
            .or(self.panel.as_mut())
        {
            panel.rows(inner, cap, p)
        } else {
            (vec![], vec![])
        };
        overlay.truncate(cap);
        overlay_hits.truncate(cap);
        let history_h = chat_h - overlay.len();
        let below = matches!(style, InputStyle::Lines | InputStyle::Boxed);
        let start = history_h
            + gap
            + match style {
                InputStyle::Lines | InputStyle::Boxed => extra + input_h,
                InputStyle::Fill => input_h + 2,
                InputStyle::Bar => input_h,
            };
        self.overlay_rows = start..start + overlay.len();
        self.chat_pad = pad;
        self.chat_hits.clear();
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
            let (view, clamped) = self.chat.viewport(inner, p, self.scroll, history_h);
            self.scroll = clamped;
            screen.resize_with(history_h - view.len(), Vec::new);
            self.chat_hits.resize_with(screen.len(), || None);
            for row in view {
                screen.push(row.line);
                self.chat_hits.push(row.hit);
            }
        }
        if gap == 1 {
            screen.push(Vec::new());
        }
        view.rows
            .resize_with(view.rows.len().max(input_h), String::new);
        let shown = &view.rows[input_top..input_top + input_h];
        let tail = if below {
            vec![]
        } else {
            std::mem::take(&mut overlay)
        };
        match style {
            InputStyle::Boxed => {
                screen.extend(self.boxed_input(&view, editor_w, shown, input_top));
            }
            InputStyle::Fill => {
                screen.extend(self.fill_input(&view, editor_w, shown, input_top, tail));
            }
            InputStyle::Bar => {
                screen.extend(self.bar_input(&view, editor_w, shown, input_top, tail));
            }
            InputStyle::Lines => screen.extend(
                self.input_block(&view, editor_w, shown, input_top)
                    .into_iter()
                    .skip(1 - lead)
                    .take(extra + input_h),
            ),
        }
        screen.extend(overlay);
        screen.truncate(h);
        self.chat_hits.resize_with(self.overlay_rows.start, || None);
        self.chat_hits.extend(overlay_hits);
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
        if let Some(m) = self.selection.message
            && is_overlay(m.id)
            && !self.chat_hits.iter().flatten().any(|h| h.id == m.id)
        {
            self.selection.message = None;
        }
        if let Some(selection) = self.selection.message {
            selection.render(&self.chat_hits, &mut screen, Style::fg(p.bg).on(p.accent));
        }

        let first = history_h + gap + lead;
        self.input_area = Some(crate::ui::app::InputArea {
            row: first,
            col: input_col,
            height: input_h,
            top: input_top,
            cols: text_w,
            boxed: style == InputStyle::Boxed,
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
        assert_eq!((area.row, area.col, area.height, area.top), (3, 2, 4, 0));
        assert_eq!(frame.cursor, Some((6, 6)));
        assert!(frame.screen[2..].iter().all(|row| line_width(row) == 80));
        assert!(frame.screen[2][0].text.starts_with('╭'));
        assert!(text(&frame.screen[3]).starts_with("│ one"));
        assert!(frame.screen[7][1].text.trim().is_empty());
        assert!(text(&frame.screen[8]).starts_with("│ Waiting"));
        assert!(text(&frame.screen[8]).ends_with(" │"));
        assert!(frame.screen[9][0].text.starts_with('╰'));
        assert!(
            !app.compose(80, 4)
                .screen
                .iter()
                .flatten()
                .any(|s| s.text.starts_with('╭'))
        );
        assert!(app.compose(80, 10).screen[2][0].text.starts_with('╭'));
        assert_eq!(app.editor.text(), "one\ntwo\nthree\nfour");
        cleanup(app);
    }

    #[test]
    fn boxed_status_clips_both_labels_inside_the_padding() {
        let mut app = test_app("boxed-status");
        app.palette.ui.input_style = InputStyle::Boxed;
        app.session.set_model("model-abcdefghijklmnop", "default");
        app.display_flash("waiting for a reply");
        let frame = app.compose(20, 6);
        assert_eq!(text(&frame.screen[4]), "│ wait…   model-a… │");
        cleanup(app);
    }

    #[test]
    fn boxed_requires_two_content_columns_after_margins_and_padding() {
        let mut app = test_app("boxed-width");
        app.palette.ui.input_style = InputStyle::Boxed;
        app.palette.ui.pad_x = 1;
        app.editor.insert("界z");
        let frame = app.compose(8, 8);
        let area = app.input_area.unwrap();
        assert_eq!((area.col, area.cols), (3, 2));
        assert_eq!(text(&frame.screen[area.row]), "│ 界 │");
        let narrow = app.compose(7, 8);
        assert!(!narrow.screen.iter().any(|row| text(row).starts_with('╭')));
        assert_eq!(app.palette.ui.input_style, InputStyle::Boxed);
        let restored = app.compose(8, 8);
        assert_eq!(restored.screen, frame.screen);
        assert_eq!(restored.cursor, frame.cursor);
        assert_eq!(app.editor.text(), "界z");
        assert_eq!(app.editor.cursor(), 2);
        cleanup(app);
    }

    #[test]
    fn fill_layout_pads_the_input_and_puts_the_status_below_a_gap() {
        let mut app = test_app("fill-layout");
        app.palette.ui.input_style = InputStyle::Fill;
        app.editor.insert("one\ntwo");
        app.display_flash("Waiting");
        let frame = app.compose(40, 10);
        let area = app.input_area.unwrap();
        assert_eq!((area.row, area.col, area.height), (5, 2, 2));
        assert_eq!(frame.cursor, Some((5, 6)));
        let bg = app.palette.border;
        for row in &frame.screen[4..=7] {
            assert_eq!(line_width(row), 40);
            assert!(row.iter().all(|s| s.style.bg == bg));
        }
        assert!(frame.screen[4].iter().all(|s| s.text.trim().is_empty()));
        assert_eq!(frame.screen[5][0].text, "› ");
        assert!(frame.screen[7].iter().all(|s| s.text.trim().is_empty()));
        assert!(frame.screen[8].iter().all(|s| s.text.is_empty()));
        assert_eq!(frame.screen[9][0].text, "Waiting");
        assert_eq!(frame.screen.len(), 10);
        assert!(
            !app.compose(40, 4)
                .screen
                .iter()
                .flatten()
                .any(|s| s.style.bg == bg && bg.is_some())
        );
        cleanup(app);
    }
    #[test]
    fn titled_picker_reserves_choices_and_pages_by_their_height() {
        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

        let mut app = test_app("titled-picker-layout");
        app.open_lua_picker(
            1,
            (0..8).map(|i| format!("choice {i}")).collect(),
            Some("**Choose one**".into()),
        );
        app.palette.ui.list_max_height = 3;
        let frame = app.compose(40, 20);
        assert!(frame.cursor.is_none());
        let overlay = &frame.screen[app.overlay_rows.clone()];
        assert_eq!(overlay.len(), 3);
        assert!(
            overlay[0]
                .iter()
                .any(|s| s.text == "Choose one" && s.style.bold)
        );
        assert!(text(&overlay[1]).contains("choice 0"));
        app.key(KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE), 20);
        assert_eq!(app.picker.as_ref().unwrap().selected, 2);
        let frame = app.compose(40, 20);
        let overlay = &frame.screen[app.overlay_rows.clone()];
        assert!(text(&overlay[0]).contains("Choose one"));
        assert!(text(&overlay[2]).contains("choice 2"));

        app.palette.ui.list_max_height = 1;
        let frame = app.compose(40, 20);
        let overlay = &frame.screen[app.overlay_rows.clone()];
        assert_eq!(overlay.len(), 1);
        assert!(text(&overlay[0]).contains("choice 2"));
        cleanup(app);
    }

    fn open_list(app: &mut crate::ui::app::App) {
        use crate::ui::picker::{Kind, Picker, item};
        let items = ["one", "two", "three"].map(|s| item(s, "", s)).to_vec();
        app.open_modal(Picker::new(Kind::Resume, items));
    }

    fn text(row: &[crate::kit::text::Span]) -> String {
        row.iter().map(|s| s.text.as_str()).collect()
    }

    #[test]
    fn every_style_has_an_empty_row_above_the_composer() {
        for (style, lead) in [
            (InputStyle::Lines, 1),
            (InputStyle::Boxed, 1),
            (InputStyle::Fill, 1),
            (InputStyle::Bar, 0),
        ] {
            let mut app = test_app("gap-layout");
            app.palette.ui.input_style = style;
            app.notice("history");
            let frame = app.compose(40, 14);
            let row = app.input_area.unwrap().row - lead - 1;
            assert!(
                frame.screen[row].iter().all(|s| s.text.trim().is_empty()),
                "{style:?}"
            );
            assert_eq!(frame.screen.len(), 14);
            cleanup(app);
        }
    }

    #[test]
    fn lists_open_below_the_input() {
        let mut app = test_app("list-below");
        app.palette.ui.input_style = InputStyle::Boxed;
        open_list(&mut app);
        let frame = app.compose(40, 20);
        let bottom = frame
            .screen
            .iter()
            .position(|r| text(r).starts_with('╰'))
            .unwrap();
        assert_eq!(app.overlay_rows.start, bottom + 1);
        assert!(text(&frame.screen[bottom + 1]).contains("one"));
        assert_eq!(frame.screen.len(), 20);

        app.palette.ui.input_style = InputStyle::Fill;
        let frame = app.compose(40, 20);
        let list = app.overlay_rows.clone();
        assert!(text(&frame.screen[list.start]).contains("one"));
        assert!(text(&frame.screen[list.end]).starts_with('─'));
        assert_eq!(text(&frame.screen[list.start - 1]).trim(), "");
        assert!(
            frame.screen[list.start - 1]
                .iter()
                .all(|s| s.style.bg == app.palette.border)
        );
        assert_eq!(frame.screen.len(), 20);
        cleanup(app);
    }

    #[test]
    fn bar_rows_start_with_the_accent_bar_and_the_status_follows_a_gap() {
        let mut app = test_app("bar-layout");
        app.palette.ui.input_style = InputStyle::Bar;
        app.editor.insert("one\ntwo\nthree");
        app.display_flash("Waiting");
        let frame = app.compose(40, 12);
        let area = app.input_area.unwrap();
        assert_eq!((area.col, area.height), (2, 3));
        for (i, want) in ["one", "two", "three"].iter().enumerate() {
            let row = &frame.screen[area.row + i];
            assert_eq!(
                (row[0].text.as_str(), row[0].style.fg),
                ("▌ ", app.palette.accent)
            );
            assert_eq!(row[1].text, *want);
        }
        assert!(frame.screen[area.row + 3].is_empty());
        assert_eq!(frame.screen[area.row + 4][0].text, "Waiting");
        assert_eq!(frame.screen.len(), 12);
        open_list(&mut app);
        let frame = app.compose(40, 12);
        let list = app.overlay_rows.clone();
        let area = app.input_area.unwrap();
        assert_eq!(list.start, area.row + area.height);
        assert!(text(&frame.screen[list.end]).starts_with('─'));
        cleanup(app);
    }
    #[test]
    fn small_viewports_drop_the_gap_and_fall_back_without_losing_the_draft() {
        let mut app = test_app("small-layout");
        app.editor.insert("draft");
        app.palette.ui.input_style = InputStyle::Boxed;
        let frame = app.compose(40, 5);
        assert!(text(&frame.screen[0]).starts_with('╭'));
        app.palette.ui.input_style = InputStyle::Bar;
        let frame = app.compose(40, 4);
        assert!(!frame.screen.iter().any(|r| text(r).contains('▌')));
        assert_eq!(frame.screen.len(), 4);
        assert_eq!(app.editor.text(), "draft");
        let frame = app.compose(40, 12);
        assert!(frame.screen.iter().any(|r| text(r).starts_with("▌ draft")));
        cleanup(app);
    }
}
