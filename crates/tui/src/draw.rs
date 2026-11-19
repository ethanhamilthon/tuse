use crate::app::App;
use crate::input_view;
use crate::style::Style;
use crate::text::{Line, line_width, span};
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
    let fr = input_view::frame(t);
    let text_w = inner.saturating_sub(fr.left + fr.right).max(1);
    app.input_cols = text_w;
    let (rows, (crow, ccol)) = app.editor.layout(text_w);
    let input_h = tuse_config::limit(t.input_max_height, rows.len())
        .max(1)
        .min(h.saturating_sub(fr.head + fr.foot).max(1));
    let input_top = crow.saturating_sub(input_h - 1).min(rows.len() - input_h);
    let fixed = input_h + fr.head + fr.foot;
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
    let cap = chat_h.min(t.list_max_height as usize);
    if let Some(panel) = &mut app.panel {
        let lines = panel.lines(inner, cap, p);
        view.truncate(chat_h.saturating_sub(lines.len()));
        view.extend(lines);
    }
    let lines = crate::list::picker(app, inner, cap);
    let p = &app.palette;
    view.truncate(chat_h.saturating_sub(lines.len()));
    view.extend(lines);
    screen.extend(view);
    let shown = &rows[input_top..input_top + input_h];
    screen.extend(input_view::block(app, &fr, inner, shown));

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
    let first = h.saturating_sub(input_h + fr.foot);
    app.input_area = Some(crate::app::InputArea {
        row: first,
        col: pad + fr.left,
        height: input_h,
        top: input_top,
        cols: text_w,
    });
    let cy = first + crow - input_top;
    out.queue(MoveTo((pad + fr.left + ccol) as u16, cy as u16))?;
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
