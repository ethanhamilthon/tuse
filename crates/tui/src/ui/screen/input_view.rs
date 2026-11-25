use crate::kit::style::{INPUT_PREFIX, MORE, Palette, RULE_CHAR, SCROLL_MARK, Style};
use crate::kit::text::{Line, clip_line, line_width, span};
use crate::ui::app::App;
use tuse_config::InputStyle;

fn label(line: Line, room: usize) -> Line {
    if room == 0 {
        vec![]
    } else {
        clip_line(&line, room, MORE)
    }
}

fn rule(p: &Palette, w: usize, mut left: Line, mut right: Line) -> Line {
    if !right.is_empty() && line_width(&left) + line_width(&right) < w {
        right.insert(0, span(" ", Style::default()));
    }
    let fill = w.saturating_sub(line_width(&left) + line_width(&right));
    left.push(span(RULE_CHAR.repeat(fill), Style::fg(p.border)));
    left.extend(right);
    left
}

impl App {
    fn input_status(&self, w: usize) -> (Line, Line, Line) {
        let p = &self.palette;
        let mark = if self.scroll > 0 {
            vec![span(
                format!("{SCROLL_MARK}{} ", self.scroll),
                Style::fg(p.dim),
            )]
        } else {
            vec![]
        };
        let mark = label(mark, w);
        let working = match &self.flash {
            Some((text, _)) => vec![span(text, Style::fg(p.accent))],
            None if self.session.busy() => self.working(),
            None => self.cwd_label(),
        };
        (mark, working, self.model_label())
    }

    fn status_row(&self, w: usize) -> Line {
        let (mark, mut activity, model) = self.input_status(w);
        if !mark.is_empty() && !activity.is_empty() {
            activity.push(span(" ", Style::default()));
        }
        activity.extend(mark);
        let gap = 3;
        let model_room = if activity.is_empty() {
            w
        } else {
            w.saturating_sub(line_width(&activity) + gap).max(w / 2)
        };
        let model = label(model, model_room);
        let room = w.saturating_sub(line_width(&model) + if model.is_empty() { 0 } else { gap });
        let mut status = label(activity, room);
        status.push(span(
            " ".repeat(w - line_width(&status) - line_width(&model)),
            Style::default(),
        ));
        status.extend(model);
        status
    }

    pub(crate) fn fill_input(
        &self,
        view: &crate::kit::editor::View,
        w: usize,
        rows: &[String],
        top: usize,
    ) -> Vec<Line> {
        let bg = Style::default().on(self.palette.border);
        let blank = || vec![span(" ".repeat(w), bg)];
        let mut out = vec![blank()];
        for mut row in self.input_rows(view, rows, top, InputStyle::Fill) {
            for s in &mut row {
                s.style.bg = s.style.bg.or(bg.bg);
            }
            row.push(span(" ".repeat(w.saturating_sub(line_width(&row))), bg));
            out.push(row);
        }
        out.push(blank());
        out.push(vec![]);
        out.push(self.status_row(w));
        out
    }

    pub(crate) fn boxed_input(
        &self,
        view: &crate::kit::editor::View,
        w: usize,
        rows: &[String],
        top: usize,
    ) -> Vec<Line> {
        let border = Style::fg(self.palette.border);
        let surround = |line: Line| {
            let mut out = vec![span("│", border)];
            let fill = w.saturating_sub(line_width(&line));
            out.extend(line);
            out.push(span(" ".repeat(fill), Style::default()));
            out.push(span("│", border));
            out
        };
        let mut out = vec![vec![span(format!("╭{}╮", RULE_CHAR.repeat(w)), border)]];
        out.extend(
            self.input_rows(view, rows, top, InputStyle::Boxed)
                .into_iter()
                .map(surround),
        );
        out.push(surround(vec![]));
        out.push(surround(self.status_row(w)));
        out.push(vec![span(format!("╰{}╯", RULE_CHAR.repeat(w)), border)]);
        out
    }

    pub(crate) fn input_block(
        &self,
        view: &crate::kit::editor::View,
        w: usize,
        rows: &[String],
        top: usize,
    ) -> Vec<Line> {
        let (mark, working, model) = self.input_status(w);
        let working = label(working, w.saturating_sub(line_width(&mark)));
        let mut out = vec![rule(&self.palette, w, mark, working)];
        out.extend(self.input_rows(view, rows, top, InputStyle::Lines));
        out.push(rule(&self.palette, w, vec![], label(model, w)));
        out
    }

    pub(crate) fn input_left(&self, mode: InputStyle) -> usize {
        match mode {
            InputStyle::Boxed => 0,
            _ => crate::kit::text::width(INPUT_PREFIX),
        }
    }

    fn input_rows(
        &self,
        view: &crate::kit::editor::View,
        rows: &[String],
        top: usize,
        mode: InputStyle,
    ) -> Vec<Line> {
        let p = &self.palette;
        let dim = self.steering.is_some() && self.saved_editor.is_none();
        let style = Style::fg(if dim { p.dim } else { p.user });
        let left = self.input_left(mode);
        let mut out = vec![];
        for (i, text) in rows.iter().enumerate() {
            let prefix = if i == 0 && left > 0 {
                INPUT_PREFIX.to_string()
            } else {
                " ".repeat(left)
            };
            let pfx_color = if self.input_disabled() {
                if mode == InputStyle::Fill {
                    p.muted
                } else {
                    p.border
                }
            } else {
                p.accent
            };
            let mut line = vec![];
            if !prefix.is_empty() {
                line.push(span(prefix, Style::fg(pfx_color)));
            }
            line.push(span(text, style));
            out.push(line);
        }
        if !dim {
            for (line, ranges) in out.iter_mut().zip(view.atoms.iter().skip(top)) {
                for &(a, b) in ranges {
                    crate::kit::selection::highlight(line, left + a, left + b, Style::fg(p.accent));
                }
            }
        }
        for (line, &(a, b)) in out.iter_mut().zip(view.selected.iter().skip(top)) {
            crate::kit::selection::highlight(
                line,
                left + a,
                left + b,
                Style::fg(p.bg).on(p.accent),
            );
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rules_fit_and_the_first_row_has_a_prompt() {
        let mut app = crate::ui::app::test_app("input-rules");
        app.scroll = 1000;
        app.flash = Some(("x".repeat(200), std::time::Instant::now()));
        for w in [1, 20, 41, 100] {
            let view = app.editor.view(app.input_cols.max(1));
            let out = app.input_block(&view, w, &["hello".into(), "world".into()], 0);
            assert_eq!(line_width(&out[0]), w);
            assert_eq!(line_width(&out[3]), w);
            assert_eq!(out[1][0].text, INPUT_PREFIX);
            assert_eq!(out[2][0].text, "  ");
        }
        crate::ui::app::cleanup(app);
    }

    #[test]
    fn idle_status_shows_the_muted_cwd_in_the_indicator_slot() {
        let mut app = crate::ui::app::test_app("input-cwd");
        let cwd = app.cwd_label()[0].text.clone();
        assert!(!cwd.is_empty());
        let view = app.editor.view(40);
        let lines = app.input_block(&view, 120, &["x".into()], 0);
        let top = lines[0].last().unwrap();
        assert_eq!(
            (top.text.as_str(), top.style.fg),
            (cwd.as_str(), app.palette.muted)
        );
        app.palette.ui.input_style = InputStyle::Boxed;
        let boxed = app.boxed_input(&view, 120, &["x".into()], 0);
        assert_eq!(boxed[3][1].text, cwd);
        app.flash("Copied.");
        let flashed = app.input_block(&view, 120, &["x".into()], 0);
        assert!(!flashed[0].iter().any(|s| s.text == cwd));
        crate::ui::app::cleanup(app);
    }

    #[test]
    fn picked_atoms_are_accent_and_typed_tokens_are_not() {
        let mut app = crate::ui::app::test_app("input-atoms");
        app.input_cols = 60;
        app.editor.insert("#typed ");
        app.editor.insert_atom(
            "#review",
            crate::kit::editor::AtomKind::Prompt("review".into(), None),
        );
        app.editor.insert(" x");
        let view = app.editor.view(60);
        let rows = view.rows.clone();
        let out = app.input_block(&view, 80, &rows, 0);
        let accent: Vec<_> = out[1][1..]
            .iter()
            .filter(|s| s.style.fg == app.palette.accent && s.style.fg.is_some())
            .map(|s| s.text.as_str())
            .collect();
        assert_eq!(accent, ["#review"]);
        crate::ui::app::cleanup(app);
    }
}
