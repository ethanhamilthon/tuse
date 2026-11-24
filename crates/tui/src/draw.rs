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

#[derive(Default)]
pub struct Renderer {
    prev: Vec<Line>,
    size: (usize, usize),
    palette_gen: u64,
}

impl Renderer {
    pub fn draw(
        &mut self,
        app: &mut App,
        out: &mut impl Write,
        w: u16,
        h: u16,
    ) -> std::io::Result<()> {
        let (w, h) = (w as usize, h as usize);
        if self.size != (w, h) || self.palette_gen != app.palette_gen {
            app.selection = Default::default();
            app.clear_link_hover();
            self.prev.clear();
            self.size = (w, h);
            self.palette_gen = app.palette_gen;
        }
        let p = &app.palette;
        let ui = &p.ui;
        let pad = ui.pad_x as usize;
        let inner = w.saturating_sub(pad * 2).max(1);
        let left = crate::text::width(crate::style::INPUT_PREFIX);
        let text_w = inner.saturating_sub(left).max(1);
        app.input_cols = text_w;
        let view = app.editor.view(text_w);
        let (crow, ccol) = view.cursor;
        let rows = view.rows.clone();
        let head = usize::from(h > 1);
        let foot = usize::from(h > 2);
        let editor_room = h.saturating_sub(head + foot).max(1);
        let input_h = tuse_config::limit(ui.input_max_height, rows.len())
            .max(1)
            .min(editor_room);
        let input_top = crow
            .saturating_sub(input_h.saturating_sub(1))
            .min(rows.len().saturating_sub(input_h));
        let fixed = input_h + head + foot;
        let chat_h = h.saturating_sub(fixed);
        let mut screen: Vec<Line> = Vec::with_capacity(h);

        let cap = (chat_h - usize::from(chat_h > 2)).min(p.ui.list_max_height);
        let mut overlay = if app.picker.is_some() {
            {
                let p = &app.palette;
                match &mut app.picker {
                    Some(pk) => {
                        let rows = cap.saturating_sub(1).max(1);
                        let lines = crate::list::options(pk, rows, inner, p);
                        app.list_rows = rows;
                        crate::list::framed(lines, inner, p)
                    }
                    None => vec![],
                }
            }
        } else if let Some(panel) = app
            .question
            .as_mut()
            .map(|q| &mut q.panel)
            .or(app.panel.as_mut())
        {
            panel.lines(inner, cap, p)
        } else {
            vec![]
        };
        overlay.truncate(cap);
        let history_h = chat_h - overlay.len();
        app.overlay_rows = history_h..chat_h;
        let p = &app.palette;
        if history_h > 0 {
            let (view, clamped) = app.chat.window(inner, p, app.scroll, history_h);
            app.scroll = clamped;
            screen.resize_with(history_h - view.len(), Vec::new);
            screen.extend(view);
        }
        screen.extend(overlay);
        let mut rows = rows;
        rows.resize_with(rows.len().max(input_h), String::new);
        let shown = &rows[input_top..input_top + input_h];
        screen.extend(
            input_view::block(app, &view, inner, shown, input_top)
                .into_iter()
                .skip(1 - head)
                .take(head + input_h + foot),
        );
        screen.truncate(h);
        app.links = crate::links::scan(&screen, pad);
        if let Some(id) = app.hover
            && app.pointer.is_none_or(|pt| app.link_at(pt) != Some(id))
        {
            app.hover = None;
        }
        if let Some((pt, id)) = app.pressed_link
            && app.link_at(pt) != Some(id)
        {
            app.pressed_link = None;
        }
        crate::links::highlight(
            &mut screen,
            app.pressed_link.map(|(_, id)| id).or(app.hover),
            p.fg,
        );
        app.selection
            .render(&mut screen, pad, Style::fg(p.bg).on(p.accent));

        out.queue(BeginSynchronizedUpdate)?.queue(Hide)?;
        let base = Style::fg(p.fg).on(p.bg);
        for (y, line) in screen.iter().enumerate() {
            if self.prev.get(y) == Some(line) {
                continue;
            }
            out.queue(MoveTo(0, y as u16))?;
            let mut cur = None;
            if pad > 0 {
                put(out, &span(" ".repeat(pad), base), base, &mut cur)?;
            }
            for s in line {
                put(out, s, base, &mut cur)?;
            }
            let used = pad + line_width(line);
            if used < w {
                apply_style(out, base, &mut cur)?;
                out.queue(Clear(ClearType::UntilNewLine))?;
            }
            out.queue(SetAttribute(Attribute::Reset))?
                .queue(ResetColor)?;
        }
        self.prev = screen;

        let first = h.saturating_sub(input_h + foot);
        app.input_area = Some(crate::app::InputArea {
            row: first,
            col: pad + left,
            height: input_h,
            top: input_top,
            cols: text_w,
        });
        if !app.input_disabled() {
            let cy = (first + crow - input_top).min(h.saturating_sub(1));
            out.queue(MoveTo((pad + left + ccol) as u16, cy as u16))?;
            out.queue(Show)?;
        }
        out.queue(EndSynchronizedUpdate)?;
        out.flush()
    }
}

fn put(
    out: &mut impl Write,
    s: &crate::text::Span,
    base: Style,
    cur: &mut Option<Style>,
) -> std::io::Result<()> {
    let eff = Style {
        fg: if s.style.link.is_some() {
            s.style.fg
        } else {
            s.style.fg.or(base.fg)
        },
        bg: s.style.bg.or(base.bg),
        bold: s.style.bold,
        italic: s.style.italic,
        dim: s.style.dim,
        underline: s.style.underline,
        link: None,
    };
    apply_style(out, eff, cur)?;
    out.queue(Print(&s.text))?;
    Ok(())
}

fn apply_style(out: &mut impl Write, eff: Style, cur: &mut Option<Style>) -> std::io::Result<()> {
    if *cur == Some(eff) {
        return Ok(());
    }
    let need_reset = match *cur {
        None => true,
        Some(prev) => {
            (prev.bold && !eff.bold)
                || (prev.italic && !eff.italic)
                || (prev.dim && !eff.dim)
                || (prev.underline && !eff.underline)
                || (prev.fg.is_some() && eff.fg.is_none())
                || (prev.bg.is_some() && eff.bg.is_none())
        }
    };
    let prev = if need_reset {
        out.queue(SetAttribute(Attribute::Reset))?
            .queue(ResetColor)?;
        Style::default()
    } else {
        cur.unwrap_or_default()
    };
    if eff.fg != prev.fg
        && let Some(c) = eff.fg
    {
        out.queue(SetForegroundColor(c))?;
    }
    if eff.bg != prev.bg
        && let Some(c) = eff.bg
    {
        out.queue(SetBackgroundColor(c))?;
    }
    for (on, was, a) in [
        (eff.bold, prev.bold, Attribute::Bold),
        (eff.italic, prev.italic, Attribute::Italic),
        (eff.dim, prev.dim, Attribute::Dim),
        (eff.underline, prev.underline, Attribute::Underlined),
    ] {
        if on && !was {
            out.queue(SetAttribute(a))?;
        }
    }
    *cur = Some(eff);
    Ok(())
}
