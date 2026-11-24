use crate::kit::style::Style;
use crate::kit::text::{Line, line_width, span};
use crate::ui::app::App;
use crate::ui::screen::compose::Frame;
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
        if self.size != (w, h) || self.palette_gen != app.palette_gen() {
            app.reset_for_resize();
            self.prev.clear();
            self.size = (w, h);
            self.palette_gen = app.palette_gen();
        }
        let Frame {
            screen,
            pad,
            base,
            cursor,
            ..
        } = app.compose(w, h);
        out.queue(BeginSynchronizedUpdate)?.queue(Hide)?;
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

        if let Some((x, y)) = cursor {
            out.queue(MoveTo(x, y))?;
            out.queue(Show)?;
        }
        out.queue(EndSynchronizedUpdate)?;
        out.flush()
    }
}

fn put(
    out: &mut impl Write,
    s: &crate::kit::text::Span,
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
