use crate::panel_list::List;
use crate::render::plain;
use crate::style::{Palette, Style};
use crate::text::{Line, line_width, span};
use tuse_core::TraceItem;

/// Read-only text over the bottom of the chat (`/usage`, `/trace`, a task's output).
/// It is not part of the conversation; Esc closes it.
pub struct Panel {
    pub title: String,
    text: String,
    /// First visible content line.
    pub scroll: usize,
    cache: Option<(usize, Vec<Line>)>,
    list: Option<List>,
}

impl Panel {
    pub fn new(title: impl Into<String>, text: impl Into<String>) -> Self {
        Panel {
            title: title.into(),
            text: text.into(),
            scroll: 0,
            cache: None,
            list: None,
        }
    }

    /// A list of items; Enter opens the selected one in full.
    pub fn list(title: impl Into<String>, items: Vec<TraceItem>) -> Self {
        let mut p = Panel::new(title, "");
        p.list = Some(List::new(items));
        p
    }

    /// Whether the rows of the list (not an opened item) are shown.
    pub fn is_list(&self) -> bool {
        self.list.as_ref().is_some_and(|l| !l.open)
    }

    pub fn move_by(&mut self, delta: isize) {
        if let Some(l) = &mut self.list {
            l.move_by(delta);
        }
    }

    /// Opens the selected item of the list.
    pub fn open(&mut self) {
        if let Some(l) = self.list.as_mut().filter(|l| !l.open) {
            let Some(it) = l.items.get(l.selected) else {
                return;
            };
            self.text = it.text.clone();
            l.open = true;
            self.scroll = 0;
            self.cache = None;
        }
    }

    /// From an opened item back to the list. False when there is nothing to go back to.
    pub fn back(&mut self) -> bool {
        match self.list.as_mut().filter(|l| l.open) {
            Some(l) => {
                l.open = false;
                true
            }
            None => false,
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
    /// The first one is a rule that sets the panel apart from the chat.
    pub fn lines(&mut self, width: usize, max_rows: usize, p: &Palette) -> Vec<Line> {
        let t = &p.t;
        let pad = " ".repeat(t.panel_pad_x as usize);
        let inner = width.saturating_sub(pad.len() * 2).max(1);
        let rows = max_rows.min(t.panel_max_height as usize).max(2);
        let body = rows - 2;
        let (all, head) = match &mut self.list {
            Some(l) if !l.open => {
                let all = l.rows(inner, p);
                l.follow(body.min(all.len()));
                let head = format!("{} {}/{}", self.title, l.selected + 1, all.len());
                (all, head)
            }
            _ => {
                let all = self.wrapped(inner, p).to_vec();
                let head = match self
                    .list
                    .as_ref()
                    .and_then(|l| l.items.get(l.selected).map(|it| (l.selected, it.role)))
                {
                    Some((i, role)) => format!("{} {i} {role}", self.title),
                    None => self.title.clone(),
                };
                (all, head)
            }
        };
        let total = all.len();
        let body = body.min(total);
        let from = match &self.list {
            Some(l) if !l.open => l.top,
            _ => {
                self.scroll = self.scroll.min(total.saturating_sub(body));
                self.scroll
            }
        };
        let more = if total > body && !self.is_list() {
            format!(" {}/{}", from + body, total)
        } else {
            String::new()
        };
        let base = Style::fg(p.panel_fg).on(p.panel_bg);
        let mut out: Vec<Line> = vec![vec![span(
            format!("{pad}{head}{more}"),
            Style::fg(p.panel_title).on(p.panel_bg).bold(),
        )]];
        for l in all.iter().skip(from).take(body) {
            let mut line = vec![span(pad.clone(), base)];
            line.extend(l.iter().map(|s| {
                let mut s = s.clone();
                s.style.bg = s.style.bg.or(p.panel_bg);
                s
            }));
            out.push(line);
        }
        for l in &mut out {
            let fill = width.saturating_sub(line_width(l));
            l.push(span(" ".repeat(fill), base));
        }
        let rule = Style::fg(p.rule);
        out.insert(0, vec![span(t.rule_char.repeat(width), rule)]);
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
        assert_eq!(panel.scroll, 30 - 6);
        assert_eq!(last.len(), 8);
        let short = Panel::new("T", "one").lines(20, 8, &p);
        assert_eq!(short.len(), 3);
    }
}

#[cfg(test)]
mod list_tests {
    use super::*;
    use tuse_config::Theme;

    fn items(n: usize) -> Vec<TraceItem> {
        (0..n)
            .map(|i| TraceItem {
                role: "user",
                summary: format!("item {i}"),
                text: format!("full text {i}\nsecond line"),
            })
            .collect()
    }

    #[test]
    fn list_selects_opens_and_goes_back() {
        let p = Palette::new(&Theme::default());
        let mut panel = Panel::list("Trace", items(30));
        let rows = panel.lines(40, 8, &p);
        assert_eq!(rows.len(), 8);
        // the last item is selected and visible
        let text = |l: &Line| l.iter().map(|s| s.text.clone()).collect::<String>();
        assert!(rows.iter().any(|l| text(l).contains("item 29")));
        panel.move_by(-5);
        panel.open();
        let open = panel.lines(40, 8, &p);
        assert!(open.iter().any(|l| text(l).contains("full text 24")));
        assert!(panel.back());
        assert!(panel.is_list());
        assert!(!panel.back());
    }
}
