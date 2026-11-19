use crate::app::App;
use crate::input_view;
use crate::style::Style;
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
    let fr = input_view::frame(t);
    let text_w = inner.saturating_sub(fr.left + fr.right).max(1);
    let (rows, (crow, ccol)) = app.editor.layout(text_w);
    let input_h = rows.len().clamp(1, (t.input_max_height as usize).max(1));
    let input_top = crow.saturating_sub(input_h - 1).min(rows.len() - input_h);
    let picker = picker_lines(app, inner);
    let fixed = input_h + fr.head + fr.foot + picker.len();
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
    if let Some(panel) = &mut app.panel {
        let lines = panel.lines(inner, chat_h, p);
        view.truncate(chat_h - lines.len());
        view.extend(lines);
    }
    screen.extend(view);
    screen.extend(picker);
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
    let cy = h.saturating_sub(input_h + fr.foot) + crow - input_top;
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
