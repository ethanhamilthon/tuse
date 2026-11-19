use crate::render::plain;
use crate::style::{Palette, Style};
use crate::text::Line;

/// Read-only text over the bottom of the chat (`/usage`, a task's output, an opened
/// `/trace` item). It is not part of the conversation; Esc closes it.
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
            let lines = plain(&self.text, width, Style::fg(p.list_fg), "", "");
            self.cache = Some((width, lines));
        }
        &self.cache.as_ref().unwrap().1
    }

    /// The panel as screen lines, at most `max_rows` of them, each `width` wide.
    pub fn lines(&mut self, width: usize, max_rows: usize, p: &Palette) -> Vec<Line> {
        let pad = p.t.list_pad_x as usize;
        let inner = width.saturating_sub(pad * 2).max(1);
        let body = max_rows.min(p.t.list_max_height as usize).max(3) - 2;
        let all = self.wrapped(inner, p).to_vec();
        let body = body.min(all.len());
        self.scroll = self.scroll.min(all.len().saturating_sub(body));
        let more = if all.len() > body {
            format!(" {}/{}", self.scroll + body, all.len())
        } else {
            String::new()
        };
        let rows = all.into_iter().skip(self.scroll).take(body).collect();
        crate::list::framed(&format!("{}{more}", self.title), rows, width, p)
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
        assert_eq!(panel.scroll, 30 - 6);
        assert_eq!(last.len(), 8);
        let short = Panel::new("T", "one").lines(20, 8, &p);
        assert_eq!(short.len(), 3);
    }
}
