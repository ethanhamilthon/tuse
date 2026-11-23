use crate::style::Style;
use crate::text::{Line, char_width, span};

#[derive(Default)]
pub struct Selection {
    pub drag: Option<((usize, usize), (usize, usize))>,
    pub input: Option<usize>,
    pub text: String,
}
impl Selection {
    pub fn render(&mut self, screen: &mut [Line], pad: usize, style: Style) {
        self.text.clear();
        let Some((a, b)) = self.drag else {
            return;
        };
        if a == b {
            return;
        }
        let (a, b) = (a.min(b), a.max(b));
        for (y, line) in screen
            .iter_mut()
            .enumerate()
            .take(b.0.saturating_add(1))
            .skip(a.0)
        {
            let start = if y == a.0 { a.1.saturating_sub(pad) } else { 0 };
            let end = if y == b.0 {
                (b.1 + 1).saturating_sub(pad)
            } else {
                usize::MAX
            };
            if y != a.0 {
                self.text.push('\n');
            }
            self.text
                .push_str(highlight(line, start, end, style).trim_end());
        }
    }
}

pub fn highlight(line: &mut Line, start: usize, end: usize, style: Style) -> String {
    let mut selected = String::new();
    if start >= end {
        return selected;
    }
    let mut col = 0;
    let mut last = false;
    for s in std::mem::take(line) {
        for c in s.text.chars() {
            let w = char_width(c);
            let inside = if w == 0 {
                last
            } else {
                col < end && col + w > start
            };
            if inside {
                selected.push(c);
            }
            let st = if inside { style } else { s.style };
            if let Some(t) = line.last_mut().filter(|t| t.style == st) {
                t.text.push(c);
            } else {
                line.push(span(c.to_string(), st));
            }
            col += w;
            last = inside;
        }
    }
    selected
}

#[test]
fn backwards_drag_copies_cells_across_padded_lines() {
    let st = Style::default();
    let mut rows = vec![vec![span("xy猫   ", st)], vec![span("tail", st)]];
    let mut sel = Selection {
        drag: Some(((1, 3), (0, 4))),
        ..Default::default()
    };
    sel.render(&mut rows, 2, st);
    assert_eq!(sel.text, "猫\nta");
}
