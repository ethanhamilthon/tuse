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
            self.prev.clear();
            self.size = (w, h);
            self.palette_gen = app.palette_gen;
        }
        let p = &app.palette;
        let ui = &p.ui;
        let pad = ui.pad_x as usize;
        let inner = w.saturating_sub(pad * 2).max(1);
        let fr = input_view::frame(ui);
        let text_w = inner.saturating_sub(fr.left + fr.right).max(1);
        app.input_cols = text_w;
        let (rows, (crow, ccol)) = app.editor.layout(text_w);
        let asking = app.ask_active();
        // Yield decoration rows first when a question and its controls barely fit.
        let reserve = if asking {
            1 + if app.ask_input.is_none() {
                1
            } else {
                app.ask_controls
            }
        } else {
            0
        };
        let decor = h.saturating_sub(reserve + 1);
        let head = fr.head.min(decor);
        let foot = fr.foot.min(decor.saturating_sub(head));
        let panel_min = if asking {
            1 + usize::from(app.ask_input.is_none())
        } else {
            0
        };
        let editor_room = h.saturating_sub(head + foot + panel_min).max(1);
        let min_editor = if app.ask_input.is_some() {
            1 + app.ask_controls
        } else {
            1
        };
        let mut input_h = tuse_config::limit(ui.input_max_height, rows.len())
            .max(min_editor)
            .min(editor_room);
        if asking && app.ask_input.is_none() && h <= 2 {
            input_h = 0;
        }
        let input_top = crow
            .saturating_sub(input_h.saturating_sub(1))
            .min(rows.len().saturating_sub(input_h));
        let fixed = input_h + head + foot;
        let chat_h = h.saturating_sub(fixed);
        let mut screen: Vec<Line> = Vec::with_capacity(h);

        let (mut view, clamped) = app.chat.window(inner, &app.palette, app.scroll, chat_h);
        app.scroll = clamped;
        let p = &app.palette;
        while view.len() < chat_h {
            view.insert(0, vec![]);
        }
        let cap = chat_h.min(p.ui.list_max_height);
        if let Some(panel) = &mut app.panel {
            let lines = panel.lines(inner, cap, p);
            view.truncate(chat_h.saturating_sub(lines.len()));
            view.extend(lines);
        }
        let lines = if asking {
            crate::ask_view::lines(app, inner, chat_h)
        } else {
            crate::list::picker(app, inner, cap)
        };
        let p = &app.palette;
        view.truncate(chat_h.saturating_sub(lines.len()));
        view.extend(lines);
        screen.extend(view);
        let mut rows = rows;
        rows.resize_with(rows.len().max(input_h), String::new);
        let shown = &rows[input_top..input_top + input_h];
        screen.extend(
            input_view::block(app, &fr, inner, shown, input_top)
                .into_iter()
                .skip(fr.head - head)
                .take(head + input_h + foot),
        );
        screen.truncate(h);
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
            col: pad + fr.left,
            height: input_h,
            top: input_top,
            cols: text_w,
        });
        let cy = (first + crow - input_top).min(h.saturating_sub(1));
        out.queue(MoveTo((pad + fr.left + ccol) as u16, cy as u16))?;
        if app.picker.as_ref().is_none_or(|p| !p.is_free()) {
            out.queue(Show)?;
        }
        out.queue(EndSynchronizedUpdate)?;
        out.flush()
    }
}

#[cfg(test)]
#[path = "ask_layout_tests.rs"]
mod ask_layout_tests;

fn put(
    out: &mut impl Write,
    s: &crate::text::Span,
    base: Style,
    cur: &mut Option<Style>,
) -> std::io::Result<()> {
    let eff = Style {
        fg: s.style.fg.or(base.fg),
        bg: s.style.bg.or(base.bg),
        bold: s.style.bold,
        italic: s.style.italic,
        dim: s.style.dim,
        underline: s.style.underline,
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
