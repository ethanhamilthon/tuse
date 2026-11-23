use crate::app::App;
use crate::status;
use crate::style::{
    BAR_CHAR, BOX_CHARS, EDGE_BOTTOM, EDGE_TOP, INPUT_PREFIX, MORE, Palette, RULE_CHAR,
    SCROLL_MARK, Style,
};
use crate::text::{Line, Span, clip_line, line_width, span, width};
use tuse_config::UiSettings;

/// What the input design adds around the editor text.
pub struct Frame {
    /// Columns left and right of the text.
    pub left: usize,
    pub right: usize,
    /// Rows above the text and below it (the status rows are part of these).
    pub head: usize,
    pub foot: usize,
}

pub fn frame(ui: &UiSettings) -> Frame {
    let pw = width(INPUT_PREFIX);
    let (left, right, head, foot) = match ui.input_design.as_str() {
        "box" => (width(BOX_CHARS[4]) + pw, width(BOX_CHARS[4]), 1, 1),
        "fill" => (pw, 0, 1, 2),
        "bar" => (width(BAR_CHAR) + 1 + pw, 0, 0, 2),
        _ => (pw, 0, 1, 1),
    };
    Frame {
        left,
        right,
        head,
        foot,
    }
}

/// The input block: its frame, the visible editor `rows` and the status bars.
pub fn block(app: &App, fr: &Frame, w: usize, rows: &[String], top: usize) -> Vec<Line> {
    let p = &app.palette;
    let working = match &app.flash {
        Some((text, _)) => vec![span(text, Style::fg(p.accent))],
        None => status::working(app),
    };
    let model = status::model(app);
    let mark = scroll_mark(p, app.scroll);
    let marked: Line = mark.iter().chain(&working).cloned().collect();
    // A search typed into the editor is not the queued message.
    let steering = app.steering.is_some() && app.saved_editor.is_none();
    let body =
        |decor: &dyn Fn(usize, usize) -> (Vec<crate::text::Span>, Vec<crate::text::Span>)| {
            rows.iter()
                .enumerate()
                .map(|(i, r)| row(p, fr, w, i, r, steering, decor(i, rows.len())))
                .collect::<Vec<_>>()
        };
    let mut out = vec![];
    match p.ui.input_design.as_str() {
        "box" => {
            out.push(border(p, w, (BOX_CHARS[0], BOX_CHARS[1]), &mark, &working));
            let side = Style::fg(p.border);
            out.extend(body(&|_, _| {
                (
                    vec![span(BOX_CHARS[4], side)],
                    vec![span(BOX_CHARS[4], side)],
                )
            }));
            out.push(border(p, w, (BOX_CHARS[2], BOX_CHARS[3]), &[], &model));
        }
        "fill" => {
            out.push(edge(p, w, EDGE_TOP));
            out.extend(body(&|_, _| (vec![], vec![])));
            out.push(edge(p, w, EDGE_BOTTOM));
            out.push(status_row(w, &marked, &model));
        }
        "bar" => {
            let bar = || span(format!("{BAR_CHAR} "), Style::fg(p.accent));
            out.extend(
                rows.iter()
                    .enumerate()
                    .map(|(i, r)| row(p, fr, w, i, r, steering, (vec![bar()], vec![]))),
            );
            out.push(vec![]);
            out.push(status_row(w, &marked, &model));
        }
        _ => {
            out.push(border(p, w, ("", ""), &mark, &working));
            out.extend(body(&|_, _| (vec![], vec![])));
            out.push(border(p, w, ("", ""), &[], &model));
        }
    }
    for (line, (a, b)) in out.iter_mut().skip(fr.head).zip(
        app.editor
            .selected_columns(app.input_cols)
            .into_iter()
            .skip(top),
    ) {
        crate::selection::highlight(line, fr.left + a, fr.left + b, Style::fg(p.bg).on(p.accent));
    }
    out
}

/// `↑N ` in `dim` while the chat is scrolled up, else nothing.
fn scroll_mark(p: &Palette, scroll: usize) -> Line {
    if scroll > 0 {
        vec![span(format!("{SCROLL_MARK}{scroll} "), Style::fg(p.dim))]
    } else {
        vec![]
    }
}

/// One editor row between the decorations; `fill` pads it to the full width.
fn row(
    p: &Palette,
    fr: &Frame,
    w: usize,
    i: usize,
    text: &str,
    pending: bool,
    (left, right): (Vec<crate::text::Span>, Vec<crate::text::Span>),
) -> Line {
    let filled = p.ui.input_design == "fill";
    let bg = if filled { p.surface } else { None };
    let mut line = left;
    let prefix = if i == 0 {
        INPUT_PREFIX.to_string()
    } else {
        " ".repeat(width(INPUT_PREFIX))
    };
    line.push(span(prefix, Style::fg(p.accent)));
    let color = if pending { p.dim } else { p.fg };
    line.push(span(text, Style::fg(color)));
    if filled || p.ui.input_design == "box" {
        let used = line_width(&line) + fr.right;
        line.push(span(" ".repeat(w.saturating_sub(used)), Style::default()));
    }
    line.extend(right);
    if filled {
        for s in &mut line {
            s.style.bg = bg;
        }
    }
    line
}

/// A rule with corners; `left` and `label` sit at its two ends.
fn border(p: &Palette, w: usize, corners: (&str, &str), left: &[Span], label: &[Span]) -> Line {
    let inner = w.saturating_sub(width(corners.0) + width(corners.1));
    let left = left.to_vec();
    // A framed border keeps a space between the label and the corner.
    let tail = if corners.1.is_empty() { "" } else { " " };
    let mut label = if label.is_empty() {
        vec![]
    } else {
        let mut l = vec![span(" ", Style::default())];
        l.extend(label.iter().cloned());
        l.push(span(tail, Style::default()));
        l
    };
    label = clip_line(&label, inner.saturating_sub(line_width(&left) + 2), MORE);
    let fill = inner.saturating_sub(line_width(&left) + line_width(&label));
    let mut line = vec![span(corners.0, Style::fg(p.border))];
    line.extend(left);
    let rw = width(RULE_CHAR).max(1);
    for _ in 0..fill / rw {
        line.push(span(RULE_CHAR, Style::fg(p.border)));
    }
    line.extend(label);
    line.push(span(corners.1, Style::fg(p.border)));
    line
}

/// The half-block row above or below a filled input.
fn edge(p: &Palette, w: usize, ch: &str) -> Line {
    vec![span(ch.repeat(w), Style::fg(p.surface))]
}

/// The status row under the input: working on the left, the model on the right.
fn status_row(w: usize, working: &[Span], model: &[Span]) -> Line {
    let right = clip_line(&model.to_vec(), w, MORE);
    let room = w.saturating_sub(line_width(&right) + 1);
    let left = clip_line(&working.to_vec(), room, MORE);
    let gap = w.saturating_sub(line_width(&left) + line_width(&right));
    let mut line = left;
    line.push(span(" ".repeat(gap), Style::default()));
    line.extend(right);
    line
}

#[cfg(test)]
mod tests {
    use super::*;

    fn colored(line: &[Span]) -> Vec<(String, Option<crossterm::style::Color>)> {
        line.iter()
            .filter(|s| !s.text.trim().is_empty() && s.text != RULE_CHAR)
            .map(|s| (s.text.trim().to_string(), s.style.fg))
            .collect()
    }

    #[test]
    fn bars_keep_the_label_colors_and_fit_the_width() {
        let p = Palette::from_config(&tuse_config::Config::default());
        let working = vec![
            span("⠋ 3s", Style::fg(p.accent)),
            span(" · reading src/main.rs...", Style::fg(p.muted)),
        ];
        let model = vec![
            span("m · ", Style::fg(p.muted)),
            span("5k", Style::fg(p.accent)),
        ];
        for w in [10, 30, 80] {
            let line = border(&p, w, ("", ""), &scroll_mark(&p, 2), &working);
            assert_eq!(line_width(&line), w, "border at {w}");
            let row = status_row(w, &working, &model);
            assert_eq!(line_width(&row), w, "status row at {w}");
        }
        let line = border(&p, 80, ("", ""), &scroll_mark(&p, 2), &working);
        assert_eq!(
            colored(&line),
            [
                ("↑2".to_string(), p.dim),
                ("⠋ 3s".into(), p.accent),
                ("· reading src/main.rs...".into(), p.muted),
            ]
        );
    }
}
