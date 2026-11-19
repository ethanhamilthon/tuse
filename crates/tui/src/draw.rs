use crate::app::App;
use crate::chat::duration;
use crate::style::{Palette, Style};
use crate::text::{Line, clip, line_width, span, width};
use crossterm::QueueableCommand;
use crossterm::cursor::{Hide, MoveTo, Show};
use crossterm::style::{
    Attribute, Print, ResetColor, SetAttribute, SetBackgroundColor, SetForegroundColor,
};
use crossterm::terminal::{BeginSynchronizedUpdate, Clear, ClearType, EndSynchronizedUpdate};
use std::io::Write;

pub fn draw(app: &mut App, out: &mut impl Write, w: u16, h: u16) -> std::io::Result<()> {
    let (w, h) = (w as usize, h as usize);
    let p = &app.palette;
    let t = &p.t;
    let pad = t.pad_x as usize;
    let inner = w.saturating_sub(pad * 2).max(1);
    let (rows, (crow, ccol)) = app.editor.layout(inner);
    let input_h = rows.len().clamp(1, (t.input_max_height as usize).max(1));
    let input_top = crow.saturating_sub(input_h - 1).min(rows.len() - input_h);
    let picker = picker_lines(app, inner);
    let fixed = input_h + 2 + picker.len();
    let chat_h = h.saturating_sub(fixed);
    let mut screen: Vec<Line> = Vec::with_capacity(h);

    let mut chat = app.chat.lines(inner, &app.palette);
    let p = &app.palette;
    let max_scroll = chat.len().saturating_sub(chat_h);
    app.scroll = app.scroll.min(max_scroll);
    let end = chat.len() - app.scroll;
    let start = end.saturating_sub(chat_h);
    let mut view: Vec<Line> = chat.drain(start..end).collect();
    while view.len() < chat_h {
        view.insert(0, vec![]);
    }
    screen.extend(view);
    screen.extend(picker);
    screen.push(rule(p, inner, &top_label(app), app.scroll));
    for r in rows.iter().skip(input_top).take(input_h) {
        screen.push(vec![span(r.clone(), Style::fg(p.fg))]);
    }
    screen.push(rule(p, inner, &bottom_label(app), 0));

    out.queue(BeginSynchronizedUpdate)?.queue(Hide)?;
    let base = Style::fg(p.fg).on(p.bg);
    for (y, line) in screen.iter().take(h).enumerate() {
        out.queue(MoveTo(0, y as u16))?;
        put(out, &span(" ".repeat(pad), base), base)?;
        for s in line {
            put(out, s, base)?;
        }
        let used = pad + line_width(line);
        if let Some(bg) = base.bg {
            out.queue(SetBackgroundColor(bg))?;
        }
        if used < w {
            out.queue(Clear(ClearType::UntilNewLine))?;
        }
        out.queue(ResetColor)?;
    }
    let cy = h.saturating_sub(input_h + 1) + crow - input_top;
    out.queue(MoveTo((pad + ccol) as u16, cy as u16))?;
    if app.picker.as_ref().is_none_or(|p| p.kind.is_token()) {
        out.queue(Show)?;
    }
    out.queue(EndSynchronizedUpdate)?;
    out.flush()
}

fn put(out: &mut impl Write, s: &crate::text::Span, base: Style) -> std::io::Result<()> {
    let st = s.style;
    out.queue(SetAttribute(Attribute::Reset))?
        .queue(ResetColor)?;
    if let Some(c) = st.fg.or(base.fg) {
        out.queue(SetForegroundColor(c))?;
    }
    if let Some(c) = st.bg.or(base.bg) {
        out.queue(SetBackgroundColor(c))?;
    }
    for (on, a) in [
        (st.bold, Attribute::Bold),
        (st.italic, Attribute::Italic),
        (st.dim, Attribute::Dim),
        (st.underline, Attribute::Underlined),
    ] {
        if on {
            out.queue(SetAttribute(a))?;
        }
    }
    out.queue(Print(&s.text))?;
    out.queue(SetAttribute(Attribute::Reset))?;
    Ok(())
}

/// A horizontal rule with a label at its right end.
fn rule(p: &Palette, w: usize, label: &str, scroll: usize) -> Line {
    let t = &p.t;
    let st = Style::fg(p.rule);
    let mut left = String::new();
    if scroll > 0 {
        left = format!("{}{} {}{scroll} ", t.rule_char, t.rule_char, t.scroll_mark);
    }
    let label = if label.is_empty() {
        String::new()
    } else {
        format!(" {label}")
    };
    let label = clip(&label, w.saturating_sub(width(&left) + 2), &t.more);
    let fill = w.saturating_sub(width(&left) + width(&label));
    let mut line = vec![span(left, Style::fg(p.dim))];
    line.push(span(
        t.rule_char.repeat(fill / width(&t.rule_char).max(1)),
        st,
    ));
    line.push(span(label, Style::fg(p.status)));
    line
}

/// The turn timer; empty when the agent is idle.
fn top_label(app: &App) -> String {
    let t = &app.palette.t;
    let Some((elapsed, calls)) = app.session.turn_info() else {
        return String::new();
    };
    let frames = &t.spinner;
    let spin = frames
        .get(app.tick % frames.len().max(1))
        .cloned()
        .unwrap_or_default();
    format!("{spin} {}{}{calls} calls", duration(elapsed), t.label_sep)
}

fn bottom_label(app: &App) -> String {
    let t = &app.palette.t;
    let config = app.session.config();
    let (model, effort) = app.session.model();
    let mut parts = vec![config.display_name(&model), effort];
    let stats = app.session.stats();
    let window = config.model_def(&model).map(|d| d.window).unwrap_or(0);
    if window > 0 {
        parts.push(format!(
            "{:.0}%",
            stats.context as f64 * 100.0 / window as f64
        ));
    } else if stats.context > 0 {
        parts.push(format!("{} tok", tuse_core::tokens(stats.context)));
    }
    if stats.priced {
        parts.push(format!("${:.2}", stats.cost));
    }
    parts.join(&t.label_sep)
}

fn picker_lines(app: &App, w: usize) -> Vec<Line> {
    let Some(pk) = &app.picker else { return vec![] };
    let p = &app.palette;
    let t = &p.t;
    let ppad = " ".repeat(t.picker_pad_x as usize);
    let base = Style::fg(p.picker_fg).on(p.picker_bg);
    let mut lines = vec![];
    let fill = |mut l: Line, st: Style| {
        let n = w.saturating_sub(line_width(&l));
        l.push(span(" ".repeat(n), st));
        l
    };
    if !pk.title.is_empty() {
        let q = if pk.query.is_empty() {
            String::new()
        } else {
            format!(" › {}", pk.query)
        };
        let title = clip(&format!("{ppad}{}{q}", pk.title), w, &t.more);
        lines.push(fill(
            vec![span(
                title,
                Style::fg(p.picker_title).on(p.picker_bg).bold(),
            )],
            base,
        ));
    }
    let rows = app.picker_rows();
    if pk.shown.is_empty() {
        lines.push(fill(
            vec![span(
                format!("{ppad}no matches"),
                Style::fg(p.picker_detail).on(p.picker_bg),
            )],
            base,
        ));
    }
    for (n, &i) in pk.shown.iter().enumerate().skip(pk.scroll).take(rows) {
        let it = &pk.items[i];
        let sel = n == pk.selected;
        let st = if sel {
            Style::fg(p.picker_selected_fg).on(p.picker_selected_bg)
        } else {
            base
        };
        let det = if sel {
            st
        } else {
            Style::fg(p.picker_detail).on(p.picker_bg)
        };
        let mark = if sel {
            t.picker_selected_prefix.clone()
        } else {
            " ".repeat(width(&t.picker_selected_prefix))
        };
        let label = clip(
            &it.label,
            w.saturating_sub(ppad.len() + width(&mark)),
            &t.more,
        );
        let mut l = vec![span(format!("{ppad}{mark}{label}"), st)];
        let room = w.saturating_sub(line_width(&l) + 2);
        if !it.detail.is_empty() && room > 4 {
            l.push(span(format!("  {}", clip(&it.detail, room, &t.more)), det));
        }
        lines.push(fill(l, st));
    }
    lines
}
