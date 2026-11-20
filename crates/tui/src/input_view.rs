use crate::app::App;
use crate::motion::Motion;
use crate::status;
use crate::style::{
    BAR_CHAR, BOX_CHARS, EDGE_BOTTOM, EDGE_TOP, INPUT_PREFIX, MORE, Palette, RULE_CHAR,
    SCROLL_MARK, Style,
};
use crate::text::{Line, clip, line_width, span, width};
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
    let working = app
        .ask_input
        .as_ref()
        .map(|a| a.1.lines().collect::<Vec<_>>().join(" "))
        .or_else(|| app.flash.as_ref().map(|(text, _)| text.clone()))
        .unwrap_or_else(|| status::working(app));
    let model = status::model(app);
    let motion = app
        .session
        .turn_info()
        .and_then(|(elapsed, _)| Motion::new(p, true, elapsed));
    let m = motion.as_ref();
    let mark = scroll_mark(app.scroll);
    // A search typed into the editor is not the queued message.
    let steering = app.steering.is_some() && app.saved_editor.is_none() && app.ask_input.is_none();
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
            out.push(border(
                p,
                w,
                (BOX_CHARS[0], BOX_CHARS[1]),
                &mark,
                &working,
                m,
            ));
            let side = Style::fg(p.border);
            out.extend(body(&|_, _| {
                (
                    vec![span(BOX_CHARS[4], side)],
                    vec![span(BOX_CHARS[4], side)],
                )
            }));
            out.push(border(p, w, (BOX_CHARS[2], BOX_CHARS[3]), "", &model, m));
        }
        "fill" => {
            out.push(edge(p, w, EDGE_TOP, m));
            out.extend(body(&|_, _| (vec![], vec![])));
            out.push(edge(p, w, EDGE_BOTTOM, m));
            out.push(status_row(p, w, 0, &join(&mark, &working), &model));
        }
        "bar" => {
            let n = rows.len().max(1);
            let bar = |i: usize| {
                let color = m
                    .and_then(|m| m.along((i as f32 + 0.5) / n as f32))
                    .or(p.accent);
                span(format!("{BAR_CHAR} "), Style::fg(color))
            };
            out.extend(
                rows.iter()
                    .enumerate()
                    .map(|(i, r)| row(p, fr, w, i, r, steering, (vec![bar(i)], vec![]))),
            );
            out.push(vec![]);
            out.push(status_row(p, w, 0, &join(&mark, &working), &model));
        }
        _ => {
            out.push(border(p, w, ("", ""), &mark, &working, m));
            out.extend(body(&|_, _| (vec![], vec![])));
            out.push(border(p, w, ("", ""), "", &model, m));
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

fn join(mark: &str, working: &str) -> String {
    format!("{mark}{working}")
}

fn scroll_mark(scroll: usize) -> String {
    if scroll > 0 {
        format!("{SCROLL_MARK}{scroll} ")
    } else {
        String::new()
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
fn border(
    p: &Palette,
    w: usize,
    corners: (&str, &str),
    left: &str,
    label: &str,
    m: Option<&Motion>,
) -> Line {
    let inner = w.saturating_sub(width(corners.0) + width(corners.1));
    // A framed border keeps a space between the label and the corner.
    let tail = if corners.1.is_empty() { "" } else { " " };
    let label = if label.is_empty() {
        String::new()
    } else {
        format!(" {label}{tail}")
    };
    let label = clip(&label, inner.saturating_sub(width(left) + 2), MORE);
    let fill = inner.saturating_sub(width(left) + width(&label));
    let mut line = vec![
        span(corners.0, Style::fg(p.border)),
        span(left, Style::fg(p.dim)),
    ];
    let rw = width(RULE_CHAR).max(1);
    let mut x = width(corners.0) + width(left);
    for _ in 0..fill / rw {
        let color = m.and_then(|m| m.at(x)).or(p.border);
        line.push(span(RULE_CHAR, Style::fg(color)));
        x += rw;
    }
    line.push(span(label, Style::fg(p.accent)));
    line.push(span(corners.1, Style::fg(p.border)));
    line
}

/// The half-block row above or below a filled input.
fn edge(p: &Palette, w: usize, ch: &str, m: Option<&Motion>) -> Line {
    (0..w)
        .map(|x| {
            let color = m.and_then(|m| m.at(x)).or(p.surface);
            span(ch, Style::fg(color))
        })
        .collect()
}

/// The status row under the input: working on the left, the model on the right.
fn status_row(p: &Palette, w: usize, indent: usize, working: &str, model: &str) -> Line {
    let st = Style::fg(p.accent);
    let right = clip(model, w.saturating_sub(indent), MORE);
    let room = w.saturating_sub(indent + width(&right) + 1);
    let left = clip(working, room, MORE);
    let gap = w.saturating_sub(indent + width(&left) + width(&right));
    vec![
        span(" ".repeat(indent), st),
        span(left, st),
        span(" ".repeat(gap), st),
        span(right, st),
    ]
}
