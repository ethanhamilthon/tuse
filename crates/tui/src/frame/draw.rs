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
use std::time::{Duration, Instant};

#[derive(Default)]
pub struct Renderer {
    prev: Vec<Line>,
    size: (usize, usize),
    palette_gen: u64,
    repainted: Option<Instant>,
}

const REPAINT_EVERY: Duration = Duration::from_secs(5);

impl Renderer {
    pub fn draw(
        &mut self,
        app: &mut App,
        sink: &mut impl Write,
        w: u16,
        h: u16,
    ) -> std::io::Result<()> {
        let (w, h) = (w as usize, h as usize);
        let mut frame = Vec::with_capacity(8 << 10);
        let out = &mut frame;
        if self.size != (w, h) || self.palette_gen != app.palette_gen() {
            app.reset_for_resize();
            self.prev.clear();
            self.size = (w, h);
            self.palette_gen = app.palette_gen();
        }
        if self.repainted.is_none_or(|t| t.elapsed() >= REPAINT_EVERY) {
            self.prev.clear();
            self.repainted = Some(Instant::now());
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
        sink.write_all(&frame)?;
        sink.flush()
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
#[cfg(test)]
mod tests {
    use super::*;

    struct Writes(Vec<u8>, usize);

    impl Write for Writes {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.1 += 1;
            self.0.extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    fn frame(renderer: &mut Renderer, app: &mut App) -> (String, usize) {
        let mut out = Writes(vec![], 0);
        renderer.draw(app, &mut out, 40, 10).unwrap();
        (String::from_utf8_lossy(&out.0).into_owned(), out.1)
    }

    #[test]
    fn a_frame_is_one_write_and_everything_repaints_periodically() {
        let mut app = crate::ui::app::test_app("draw-frame");
        app.notice("hello there");
        let mut renderer = Renderer::default();
        let (first, writes) = frame(&mut renderer, &mut app);
        assert_eq!(writes, 1);
        assert!(first.contains("hello there"));
        assert!(!frame(&mut renderer, &mut app).0.contains("hello there"));
        renderer.repainted = Some(Instant::now() - REPAINT_EVERY - Duration::from_secs(1));
        assert!(frame(&mut renderer, &mut app).0.contains("hello there"));
        crate::ui::app::cleanup(app);
    }
}
