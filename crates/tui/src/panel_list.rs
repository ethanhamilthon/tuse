use crate::style::{Palette, Style};
use crate::text::{Line, clip, line_width, span, width};
use crossterm::style::Color;
use tuse_core::TraceItem;

/// The `/trace` list: one row per item, one of them selected. `open` shows its whole text.
pub struct List {
    pub items: Vec<TraceItem>,
    pub selected: usize,
    pub open: bool,
    /// First visible row of the list.
    pub top: usize,
}

impl List {
    pub fn new(items: Vec<TraceItem>) -> Self {
        let selected = items.len().saturating_sub(1);
        List {
            items,
            selected,
            open: false,
            top: 0,
        }
    }

    pub fn move_by(&mut self, delta: isize) {
        let last = self.items.len().saturating_sub(1) as isize;
        self.selected = (self.selected as isize + delta).clamp(0, last) as usize;
    }

    /// Scrolls so that the selected row is among the `body` visible ones.
    pub fn follow(&mut self, body: usize) {
        if self.selected < self.top {
            self.top = self.selected;
        } else if self.selected >= self.top + body {
            self.top = self.selected + 1 - body;
        }
        self.top = self.top.min(self.items.len().saturating_sub(body));
    }

    pub fn rows(&self, inner: usize, p: &Palette) -> Vec<Line> {
        let t = &p.t;
        let iw = self.items.len().saturating_sub(1).to_string().len();
        let mark = width(&t.picker_selected_prefix);
        self.items
            .iter()
            .enumerate()
            .map(|(i, it)| {
                let sel = i == self.selected;
                let prefix = if sel {
                    t.picker_selected_prefix.clone()
                } else {
                    " ".repeat(mark)
                };
                let head = format!("{prefix}{i:>iw$} ");
                let role = format!("{:<1$} ", it.role, t.trace_role_width as usize);
                let room = inner.saturating_sub(width(&head) + width(&role));
                let summary = clip(&it.summary, room, &t.more);
                let (a, b, c) = if sel {
                    let s = Style::fg(p.picker_selected_fg).on(p.picker_selected_bg);
                    (s, s, s)
                } else {
                    (
                        Style::fg(p.dim),
                        Style::fg(role_color(p, it.role)),
                        Style::fg(p.panel_fg),
                    )
                };
                let mut line = vec![span(head, a), span(role, b), span(summary, c)];
                if sel {
                    let fill = inner.saturating_sub(line_width(&line));
                    line.push(span(" ".repeat(fill), b));
                }
                line
            })
            .collect()
    }
}

fn role_color(p: &Palette, role: &str) -> Option<Color> {
    match role {
        "user" => p.user,
        "agent" => p.assistant,
        "thinking" => p.reasoning,
        "bash" => p.tool,
        "output" => p.tool_output,
        "error" => p.error,
        "notice" => p.notice,
        _ => p.dim,
    }
}
