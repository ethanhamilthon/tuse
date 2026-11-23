use crate::render::plain;
use crate::style::{Palette, Style};
use crate::text::{Line, wrap};

enum Body {
    Text(String),
    Markdown(String, bool),
    Lua(String),
    Styled(Vec<Line>),
}

/// Read-only text over the bottom of the chat (`/usage`, a task's output, an opened
/// `/trace` item). It is not part of the conversation; Esc closes it.
pub struct Panel {
    body: Body,
    /// First visible content line.
    pub scroll: usize,
    cache: Option<(usize, Vec<Line>)>,
}

impl Panel {
    fn with_body(body: Body) -> Self {
        Self {
            body,
            scroll: 0,
            cache: None,
        }
    }
    pub fn new(value: impl Into<String>) -> Self {
        Self::with_body(Body::Text(value.into()))
    }
    pub fn markdown(value: impl Into<String>) -> Self {
        Self::with_body(Body::Markdown(value.into(), true))
    }
    pub fn markdown_unlinked(value: impl Into<String>) -> Self {
        Self::with_body(Body::Markdown(value.into(), false))
    }
    pub fn lua(value: impl Into<String>) -> Self {
        Self::with_body(Body::Lua(value.into()))
    }
    pub fn styled(value: Vec<Line>) -> Self {
        Self::with_body(Body::Styled(value))
    }
    pub fn invalidate(&mut self) {
        self.cache = None;
    }

    /// The content wrapped to `width`.
    fn wrapped(&mut self, width: usize, p: &Palette) -> &[Line] {
        if self.cache.as_ref().is_none_or(|(w, _)| *w != width) {
            let empty = vec![];
            let lines = match &self.body {
                Body::Text(text) => plain(text, width, Style::fg(p.fg), "", ""),
                Body::Markdown(text, true) => crate::markdown::render(text, width, p),
                Body::Markdown(text, false) => {
                    crate::markdown::render_with_links(text, width, p, false)
                }
                Body::Lua(code) => {
                    let clean = crate::render::clean(code);
                    crate::syntax::highlight(&clean, Style::fg(p.fg), p)
                        .into_iter()
                        .flat_map(|r| wrap(&r, width, &empty, &empty))
                        .collect()
                }
                Body::Styled(rows) => rows
                    .iter()
                    .flat_map(|r| {
                        let r = crate::links::linkify_line(r.clone(), p);
                        wrap(&r, width, &empty, &empty)
                    })
                    .collect(),
            };
            self.cache = Some((width, lines));
        }
        &self.cache.as_ref().unwrap().1
    }

    /// The panel as screen lines, at most `max_rows` of them, each `width` wide.
    pub fn lines(&mut self, width: usize, max_rows: usize, p: &Palette) -> Vec<Line> {
        let inner = width.max(1);
        let body = max_rows.min(p.ui.list_max_height).max(2) - 1;
        let all = self.wrapped(inner, p).to_vec();
        let body = body.min(all.len());
        self.scroll = self.scroll.min(all.len().saturating_sub(body));
        let rows = all.into_iter().skip(self.scroll).take(body).collect();
        crate::list::framed(rows, width, p)
    }

    pub fn scroll_by(&mut self, delta: isize) {
        self.scroll = self.scroll.saturating_add_signed(delta);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tuse_config::{Theme, UiSettings};

    #[test]
    fn panel_scrolls_within_its_text_and_fits_the_rows() {
        let p = Palette::new(&Theme::default(), &UiSettings::default());
        let mut panel = Panel::new(
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
        let short = Panel::new("one").lines(20, 8, &p);
        assert_eq!(short.len(), 2);
    }
}
