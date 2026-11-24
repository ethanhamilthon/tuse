use crate::frame::status;
use crate::kit::style::{INPUT_PREFIX, MORE, Palette, RULE_CHAR, SCROLL_MARK, Style};
use crate::kit::text::{Line, clip_line, line_width, span};
use crate::ui::app::App;

pub fn block(
    app: &App,
    view: &crate::kit::editor::View,
    w: usize,
    rows: &[String],
    top: usize,
) -> Vec<Line> {
    let p = &app.palette;
    let half = w / 2;
    let mark = if app.scroll > 0 {
        vec![span(
            format!("{SCROLL_MARK}{} ", app.scroll),
            Style::fg(p.dim),
        )]
    } else {
        vec![]
    };
    let mark = label(mark, half);
    let working = match &app.flash {
        Some((text, _)) => vec![span(text, Style::fg(p.accent))],
        None => status::working(app),
    };
    let working = label(working, half.saturating_sub(line_width(&mark)));
    let model = label(status::model(app), half);
    let dim = app.steering.is_some() && app.saved_editor.is_none();
    let style = Style::fg(if dim { p.dim } else { p.fg });
    let left = crate::kit::text::width(INPUT_PREFIX);
    let mut out = vec![rule(p, w, mark, working)];
    for (i, text) in rows.iter().enumerate() {
        let prefix = if i == 0 {
            INPUT_PREFIX.to_string()
        } else {
            " ".repeat(left)
        };
        let pfx_color = if app.input_disabled() {
            p.border
        } else {
            p.accent
        };
        out.push(vec![span(prefix, Style::fg(pfx_color)), span(text, style)]);
    }
    if !dim {
        for (line, ranges) in out.iter_mut().skip(1).zip(view.atoms.iter().skip(top)) {
            for &(a, b) in ranges {
                crate::kit::selection::highlight(line, left + a, left + b, Style::fg(p.accent));
            }
        }
    }
    for (line, &(a, b)) in out.iter_mut().skip(1).zip(view.selected.iter().skip(top)) {
        crate::kit::selection::highlight(line, left + a, left + b, Style::fg(p.bg).on(p.accent));
    }
    out.push(rule(p, w, vec![], model));
    out
}

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
            let out = block(&app, &view, w, &["hello".into(), "world".into()], 0);
            assert_eq!(line_width(&out[0]), w);
            assert_eq!(line_width(&out[3]), w);
            assert_eq!(out[1][0].text, INPUT_PREFIX);
            assert_eq!(out[2][0].text, "  ");
        }
        crate::ui::app::cleanup(app);
    }

    #[test]
    fn picked_atoms_are_accent_and_typed_tokens_are_not() {
        let mut app = crate::ui::app::test_app("input-atoms");
        app.input_cols = 60;
        app.editor.insert("#typed ");
        app.editor.insert_atom(
            "#review",
            crate::kit::editor::AtomKind::Prompt("review".into()),
        );
        app.editor.insert(" x");
        let view = app.editor.view(60);
        let rows = view.rows.clone();
        let out = block(&app, &view, 80, &rows, 0);
        let accent: Vec<_> = out[1][1..]
            .iter()
            .filter(|s| s.style.fg == app.palette.accent && s.style.fg.is_some())
            .map(|s| s.text.as_str())
            .collect();
        assert_eq!(accent, ["#review"]);
        crate::ui::app::cleanup(app);
    }
}
