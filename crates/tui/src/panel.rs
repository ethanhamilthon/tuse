use crate::render::plain;
use crate::style::{Palette, Style};
use crate::text::{Line, line_width, span};

/// Read-only text over the bottom of the chat (`/usage`, `/trace`, a task's output).
/// It is not part of the conversation; Esc closes it.
pub struct Panel {
    pub title: String,
    text: String,
    /// First visible content line.
    pub scroll: usize,
    cache: Option<(usize, Vec<Line>)>,
}

impl Panel {
    pub fn new(title: impl Into<String>, text: impl Into<String>) -> Self {
        Panel {
            title: title.into(),
            text: text.into(),
            scroll: 0,
            cache: None,
        }
    }

    /// The text wrapped to `width`.
    fn wrapped(&mut self, width: usize, p: &Palette) -> &[Line] {
        if self.cache.as_ref().is_none_or(|(w, _)| *w != width) {
            let lines = plain(&self.text, width, Style::fg(p.panel_fg), "", "");
            self.cache = Some((width, lines));
        }
        &self.cache.as_ref().unwrap().1
    }

    /// The panel as screen lines, at most `max_rows` of them, each `width` wide.
    pub fn lines(&mut self, width: usize, max_rows: usize, p: &Palette) -> Vec<Line> {
        let t = &p.t;
        let pad = " ".repeat(t.panel_pad_x as usize);
        let inner = width.saturating_sub(pad.len() * 2).max(1);
        let rows = max_rows.min(t.panel_max_height as usize).max(1);
        let body = rows - 1;
        let total = self.wrapped(inner, p).len();
        let rows = rows.min(total + 1);
        let body = body.min(total);
        self.scroll = self.scroll.min(total.saturating_sub(body));
        let (from, base) = (self.scroll, Style::fg(p.panel_fg).on(p.panel_bg));
        let more = if total > body {
            format!(" {}/{}", from + body, total)
        } else {
            String::new()
        };
        let head = format!("{}{more}", self.title);
        let mut out: Vec<Line> = vec![vec![span(
            format!("{pad}{head}"),
            Style::fg(p.panel_title).on(p.panel_bg).bold(),
        )]];
        for l in self.wrapped(inner, p).iter().skip(from).take(rows - 1) {
            let mut line = vec![span(pad.clone(), base)];
            line.extend(l.iter().map(|s| {
                let mut s = s.clone();
                s.style.bg = p.panel_bg;
                s
            }));
            out.push(line);
        }
        for l in &mut out {
            let fill = width.saturating_sub(line_width(l));
            l.push(span(" ".repeat(fill), base));
        }
        out
    }

    pub fn scroll_by(&mut self, delta: isize) {
        self.scroll = self.scroll.saturating_add_signed(delta);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tuse_config::Theme;

    #[test]
    fn panel_scrolls_within_its_text_and_fits_the_rows() {
        let p = Palette::new(&Theme::default());
        let mut panel = Panel::new(
            "T",
            (0..30)
                .map(|i| i.to_string())
                .collect::<Vec<_>>()
                .join("\n"),
        );
        let lines = panel.lines(20, 8, &p);
        assert_eq!(lines.len(), 8);
        panel.scroll_by(1000);
        let last = panel.lines(20, 8, &p);
        assert_eq!(panel.scroll, 30 - 7);
        assert_eq!(last.len(), 8);
        let short = Panel::new("T", "one").lines(20, 8, &p);
        assert_eq!(short.len(), 2);
    }
}
