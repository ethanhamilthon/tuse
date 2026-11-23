use crate::app::App;
use crate::status;
use crate::style::{
    BAR_CHAR, BOX_CHARS, FILL_PAD, INPUT_PREFIX, MORE, Palette, RULE_CHAR, SCROLL_MARK, Style,
};
use crate::text::{Line, Span, clip_line, line_width, span, width};
use tuse_config::{InputDesign, UiSettings};

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
    let (left, right, head, foot) = match ui.input_design {
        InputDesign::Box => (width(BOX_CHARS[4]) + pw, width(BOX_CHARS[4]), 1, 1),
        InputDesign::Fill => (FILL_PAD, 0, 1, 2),
        InputDesign::Bar => (width(BAR_CHAR) + 1, 0, 0, 2),
        InputDesign::Lines => (pw, 0, 1, 1),
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
    // Each label takes at most half the width; the scroll mark counts toward the working one.
    let half = w / 2;
    let model = clip_line(&status::model(app), half, MORE);
    let mark = scroll_mark(p, app.scroll);
    let working = clip_line(&working, half.saturating_sub(line_width(&mark)), MORE);
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
    match p.ui.input_design {
        InputDesign::Box => {
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
        InputDesign::Fill => {
            out.push(edge(p, w));
            out.extend(body(&|_, _| (vec![], vec![])));
            out.push(edge(p, w));
            out.push(status_row(w, &marked, &model));
        }
        InputDesign::Bar => {
            let bar = || span(format!("{BAR_CHAR} "), Style::fg(p.accent));
            out.extend(
                rows.iter()
                    .enumerate()
                    .map(|(i, r)| row(p, fr, w, i, r, steering, (vec![bar()], vec![]))),
            );
            out.push(vec![]);
            out.push(status_row(w, &marked, &model));
        }
        InputDesign::Lines => {
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
    let design = p.ui.input_design;
    let filled = design == InputDesign::Fill;
    let bg = if filled { p.surface } else { None };
    let mut line = left;
    // `fill` and `bar` frame the input already; the others mark it with `› `.
    let prefix = match design {
        InputDesign::Fill => " ".repeat(FILL_PAD),
        InputDesign::Bar => String::new(),
        _ if i == 0 => INPUT_PREFIX.to_string(),
        _ => " ".repeat(width(INPUT_PREFIX)),
    };
    line.push(span(prefix, Style::fg(p.accent)));
    let color = if pending { p.dim } else { p.fg };
    line.push(span(text, Style::fg(color)));
    if filled || design == InputDesign::Box {
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

/// The blank row above or below a filled input: whole cells of `surface`, like the text
/// rows, so the block has straight edges (half-block glyphs came out narrower than the
/// cells and left notches at the corners).
fn edge(p: &Palette, w: usize) -> Line {
    vec![span(" ".repeat(w), Style::default().on(p.surface))]
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
    fn fill_is_a_straight_block_and_fill_bar_have_no_prefix() {
        let mut app = crate::app::test_app("fill-block");
        let w = 30;
        app.palette.ui.input_design = InputDesign::Fill;
        let surface = app.palette.surface;
        let fr = frame(&app.palette.ui);
        assert_eq!((fr.left, fr.right), (FILL_PAD, 0));
        let out = block(&app, &fr, w, &["hello".into(), "world".into()], 0);
        // Edge, two text rows, edge: every cell is `surface`, every row is `w` wide.
        for line in &out[..4] {
            assert_eq!(line_width(line), w, "{line:?}");
            assert!(line.iter().all(|s| s.style.bg == surface), "{line:?}");
            assert!(line.iter().all(|s| !s.text.contains(['▄', '▀'])));
        }
        let text: String = out[1].iter().map(|s| s.text.as_str()).collect();
        assert!(text.starts_with(" hello"), "{text:?}");
        assert!(!text.contains('›'));

        app.palette.ui.input_design = InputDesign::Bar;
        let fr = frame(&app.palette.ui);
        let out = block(&app, &fr, w, &["hello".into()], 0);
        let text: String = out[0].iter().map(|s| s.text.as_str()).collect();
        assert_eq!(text, format!("{BAR_CHAR} hello"));
        assert_eq!(
            fr.left,
            width(BAR_CHAR) + 1,
            "the cursor column follows the text"
        );

        for design in [InputDesign::Lines, InputDesign::Box] {
            app.palette.ui.input_design = design;
            let fr = frame(&app.palette.ui);
            let out = block(&app, &fr, w, &["hello".into()], 0);
            let text: String = out[fr.head].iter().map(|s| s.text.as_str()).collect();
            let at = text.find("hello").unwrap();
            assert_eq!(width(&text[..at]), fr.left, "{design:?}: {text:?}");
            assert!(text.contains(INPUT_PREFIX), "{design:?} keeps the prefix");
        }
        crate::app::cleanup(app);
    }

    #[test]
    fn labels_take_at_most_half_the_width() {
        let mut app = crate::app::test_app("half-labels");
        app.flash = Some(("x".repeat(200), std::time::Instant::now()));
        app.scroll = 3;
        for design in [
            InputDesign::Lines,
            InputDesign::Box,
            InputDesign::Fill,
            InputDesign::Bar,
        ] {
            app.palette.ui.input_design = design;
            let fr = frame(&app.palette.ui);
            for w in [20, 41, 100] {
                let out = block(&app, &fr, w, &["hi".into()], 0);
                for line in &out {
                    assert!(line_width(line) <= w, "{design:?} at {w}: {line:?}");
                }
                let text: String = out.iter().flatten().map(|s| s.text.as_str()).collect();
                let xs = text.chars().filter(|c| *c == 'x').count();
                assert!(xs + 3 <= w / 2, "{design:?} at {w}: working is {xs} + mark");
                let model: usize = out
                    .iter()
                    .flatten()
                    .filter(|s| s.style.fg == app.palette.muted)
                    .map(|s| width(&s.text))
                    .sum();
                assert!(model <= w / 2, "{design:?} at {w}: model is {model}");
            }
        }
        crate::app::cleanup(app);
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
