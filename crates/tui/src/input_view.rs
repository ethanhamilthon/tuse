use crate::app::App;
use crate::motion::Motion;
use crate::status;
use crate::style::{Palette, Style};
use crate::text::{Line, clip, line_width, span, width};
use tuse_config::Theme;

/// What the input design adds around the editor text.
pub struct Frame {
    /// Columns left and right of the text.
    pub left: usize,
    pub right: usize,
    /// Rows above the text and below it (the status rows are part of these).
    pub head: usize,
    pub foot: usize,
}

pub fn frame(t: &Theme) -> Frame {
    let pw = width(&t.input_prefix);
    let ipx = t.input_pad_x as usize;
    let (left, right, head, foot) = match t.input_design.as_str() {
        "box" => (
            width(&t.box_chars[4]) + ipx + pw,
            ipx + width(&t.box_chars[4]),
            1,
            1,
        ),
        "fill" => (ipx + pw, ipx, 1, 2),
        "bar" => (
            width(&t.bar_char) + 1 + pw,
            0,
            t.input_top_padding as usize,
            t.input_bottom_padding as usize + 2,
        ),
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
pub fn block(app: &App, fr: &Frame, w: usize, rows: &[String]) -> Vec<Line> {
    let p = &app.palette;
    let t = &p.t;
    let working = status::working(app);
    let model = status::model(app);
    let motion = app
        .session
        .turn_info()
        .and_then(|(elapsed, _)| Motion::new(p, true, elapsed));
    let m = motion.as_ref();
    let mark = scroll_mark(t, app.scroll);
    let steering = app.steering.is_some();
    let body =
        |decor: &dyn Fn(usize, usize) -> (Vec<crate::text::Span>, Vec<crate::text::Span>)| {
            rows.iter()
                .enumerate()
                .map(|(i, r)| row(p, fr, w, i, r, steering, decor(i, rows.len())))
                .collect::<Vec<_>>()
        };
    let mut out = vec![];
    match t.input_design.as_str() {
        "box" => {
            let c = |i: usize| t.box_chars[i].as_str();
            out.push(border(p, w, (c(0), c(1)), &mark, &working, m));
            let side = Style::fg(p.rule);
            let ipx = " ".repeat(t.input_pad_x as usize);
            out.extend(body(&|_, _| {
                (
                    vec![span(c(4), side), span(ipx.clone(), Style::default())],
                    vec![span(ipx.clone(), Style::default()), span(c(4), side)],
                )
            }));
            out.push(border(p, w, (c(2), c(3)), "", &model, m));
        }
        "fill" => {
            out.push(edge(p, w, &t.edge_top, m));
            let ipx = " ".repeat(t.input_pad_x as usize);
            out.extend(body(&|_, _| {
                (
                    vec![span(ipx.clone(), Style::default())],
                    vec![span(ipx.clone(), Style::default())],
                )
            }));
            out.push(edge(p, w, &t.edge_bottom, m));
            out.push(status_row(p, w, 0, &join(&mark, &working), &model));
        }
        "bar" => {
            let (top, bottom) = (
                t.input_top_padding as usize,
                t.input_bottom_padding as usize,
            );
            let n = top + rows.len() + bottom;
            let bar = |i: usize| {
                let color = m
                    .and_then(|m| m.along((i as f32 + 0.5) / n as f32))
                    .or(p.bar);
                span(format!("{} ", t.bar_char), Style::fg(color))
            };
            out.extend((0..top).map(|i| vec![bar(i)]));
            out.extend(
                rows.iter()
                    .enumerate()
                    .map(|(i, r)| row(p, fr, w, i, r, steering, (vec![bar(top + i)], vec![]))),
            );
            out.extend((0..bottom).map(|i| vec![bar(top + rows.len() + i)]));
            out.push(vec![]);
            out.push(status_row(p, w, 0, &join(&mark, &working), &model));
        }
        _ => {
            out.push(border(p, w, ("", ""), &mark, &working, m));
            out.extend(body(&|_, _| (vec![], vec![])));
            out.push(border(p, w, ("", ""), "", &model, m));
        }
    }
    out
}

fn join(mark: &str, working: &str) -> String {
    format!("{mark}{working}")
}

fn scroll_mark(t: &Theme, scroll: usize) -> String {
    if scroll > 0 {
        format!("{}{} ", t.scroll_mark, scroll)
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
    let t = &p.t;
    let filled = t.input_design == "fill";
    let bg = if filled { p.input_bg } else { None };
    let mut line = left;
    let prefix = if i == 0 {
        t.input_prefix.clone()
    } else {
        " ".repeat(width(&t.input_prefix))
    };
    line.push(span(prefix, Style::fg(p.status)));
    let color = if pending { p.dim } else { p.fg };
    line.push(span(text, Style::fg(color)));
    if filled || t.input_design == "box" {
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
    let t = &p.t;
    let inner = w.saturating_sub(width(corners.0) + width(corners.1));
    // A framed border keeps a space between the label and the corner.
    let tail = if corners.1.is_empty() { "" } else { " " };
    let label = if label.is_empty() {
        String::new()
    } else {
        format!(" {label}{tail}")
    };
    let label = clip(&label, inner.saturating_sub(width(left) + 2), &t.more);
    let fill = inner.saturating_sub(width(left) + width(&label));
    let mut line = vec![
        span(corners.0, Style::fg(p.rule)),
        span(left, Style::fg(p.dim)),
    ];
    let mut x = width(corners.0) + width(left);
    for _ in 0..fill / width(&t.rule_char).max(1) {
        let color = m.and_then(|m| m.at(x)).or(p.rule);
        line.push(span(t.rule_char.clone(), Style::fg(color)));
        x += width(&t.rule_char).max(1);
    }
    line.push(span(label, Style::fg(p.status)));
    line.push(span(corners.1, Style::fg(p.rule)));
    line
}

/// The half-block row above or below a filled input.
fn edge(p: &Palette, w: usize, ch: &str, m: Option<&Motion>) -> Line {
    (0..w)
        .map(|x| {
            let color = m.and_then(|m| m.at(x)).or(p.input_bg);
            span(ch, Style::fg(color))
        })
        .collect()
}

/// The status row under the input: working on the left, the model on the right.
fn status_row(p: &Palette, w: usize, indent: usize, working: &str, model: &str) -> Line {
    let st = Style::fg(p.status);
    let right = clip(model, w.saturating_sub(indent), &p.t.more);
    let room = w.saturating_sub(indent + width(&right) + 1);
    let left = clip(working, room, &p.t.more);
    let gap = w.saturating_sub(indent + width(&left) + width(&right));
    vec![
        span(" ".repeat(indent), st),
        span(left, st),
        span(" ".repeat(gap), st),
        span(right, st),
    ]
}
