use crate::render::plain;
use crate::style::{Palette, Style};
use crate::text::{Line, wrap};

enum Body {
    Yaml(String),
    Text(String),
    Styled(Vec<Line>),
    Markdown(String),
    Python(String),
}

/// Read-only text over the bottom of the chat (`/usage`, a task's output, an opened
/// `/trace` item). It is not part of the conversation; Esc closes it.
pub struct Panel {
    pub title: String,
    body: Body,
    /// First visible content line.
    pub scroll: usize,
    cache: Option<(usize, Vec<Line>)>,
}

impl Panel {
    fn with_body(title: impl Into<String>, body: Body) -> Self {
        Self {
            title: title.into(),
            body,
            scroll: 0,
            cache: None,
        }
    }
    pub fn new(title: impl Into<String>, value: impl Into<String>) -> Self {
        Self::with_body(title, Body::Text(value.into()))
    }
    pub fn markdown(title: impl Into<String>, value: impl Into<String>) -> Self {
        Self::with_body(title, Body::Markdown(value.into()))
    }
    pub fn python(title: impl Into<String>, value: impl Into<String>) -> Self {
        Self::with_body(title, Body::Python(value.into()))
    }
    pub fn yaml(title: impl Into<String>, value: String) -> Self {
        Self::with_body(title, Body::Yaml(value))
    }
    pub fn styled(title: impl Into<String>, value: Vec<Line>) -> Self {
        Self::with_body(title, Body::Styled(value))
    }
    pub fn invalidate(&mut self) {
        self.cache = None;
    }

    /// The content wrapped to `width`.
    fn wrapped(&mut self, width: usize, p: &Palette) -> &[Line] {
        if self.cache.as_ref().is_none_or(|(w, _)| *w != width) {
            let empty = vec![];
            let lines = match &self.body {
                Body::Yaml(text) => text
                    .lines()
                    .flat_map(|s| wrap(&crate::yaml_syntax::line(s, p), width, &empty, &empty))
                    .collect(),
                Body::Text(text) => plain(text, width, Style::fg(p.fg), "", ""),
                Body::Styled(rows) => rows
                    .iter()
                    .flat_map(|r| wrap(r, width, &empty, &empty))
                    .collect(),
                Body::Markdown(text) => crate::markdown::render(text, width, p),
                Body::Python(code) => {
                    let cleaned = crate::render::clean(code);
                    crate::syntax::highlight(&cleaned, Style::fg(p.fg), p)
                        .into_iter()
                        .flat_map(|r| wrap(&r, width, &empty, &empty))
                        .collect()
                }
            };
            self.cache = Some((width, lines));
        }
        &self.cache.as_ref().unwrap().1
    }

    /// The panel as screen lines, at most `max_rows` of them, each `width` wide.
    pub fn lines(&mut self, width: usize, max_rows: usize, p: &Palette) -> Vec<Line> {
        let inner = width.max(1);
        let body = max_rows.min(p.ui.list_max_height).max(3) - 2;
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
    use tuse_config::{Theme, UiSettings};

    #[test]
    fn panel_scrolls_within_its_text_and_fits_the_rows() {
        let p = Palette::new(&Theme::default(), &UiSettings::default());
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

    #[test]
    fn panel_highlights_markdown_and_python() {
        let p = Palette::new(&Theme::default(), &UiSettings::default());
        let mut md = Panel::markdown(
            "Trace 1/1 agent",
            "# Heading

`code`",
        );
        let md_lines = md.lines(40, 8, &p);
        assert!(md_lines.iter().flatten().any(|s| s.style.fg == p.info));
        assert!(md_lines.iter().flatten().any(|s| s.style.fg == p.ok));

        let mut py = Panel::python("Trace 1/1 python", "def f():\n    return \"hi\"");
        let py_lines = py.lines(40, 8, &p);
        assert!(py_lines.iter().flatten().any(|s| s.style.fg == p.err));
        assert!(py_lines.iter().flatten().any(|s| s.style.fg == p.ok));
    }
}
