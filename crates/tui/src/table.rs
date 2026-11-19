use crate::style::{Palette, Style};
use crate::text::{Line, line_width, span};

/// Rows of a Markdown table being collected; the first row is the header.
#[derive(Default)]
pub struct Table {
    pub rows: Vec<Vec<Line>>,
    pub row: Vec<Line>,
}

impl Table {
    /// Aligned columns; when they do not fit `width`, each row falls back to
    /// cells joined by the separator.
    pub fn render(self, width: usize, p: &Palette) -> Vec<Line> {
        let cols = self.rows.iter().map(Vec::len).max().unwrap_or(0);
        let mut widths = vec![0; cols];
        for row in &self.rows {
            for (i, c) in row.iter().enumerate() {
                widths[i] = widths[i].max(line_width(c));
            }
        }
        let sep = &p.t.table_sep;
        let total: usize =
            widths.iter().sum::<usize>() + crate::text::width(sep) * cols.saturating_sub(1);
        let fits = total <= width;
        let dim = Style::fg(p.dim);
        let mut out = vec![];
        for (r, row) in self.rows.into_iter().enumerate() {
            let mut line: Line = vec![];
            for (i, mut cell) in row.into_iter().enumerate() {
                if i > 0 {
                    line.push(span(sep.clone(), dim));
                }
                if r == 0 {
                    cell.iter_mut().for_each(|s| s.style = s.style.bold());
                }
                let pad = if fits {
                    widths[i].saturating_sub(line_width(&cell))
                } else {
                    0
                };
                line.extend(cell);
                if pad > 0 && i + 1 < cols {
                    line.push(span(" ".repeat(pad), Style::default()));
                }
            }
            out.push(line);
            if r == 0 {
                let rule =
                    p.t.rule_char
                        .repeat(if fits { total } else { width.min(total) });
                out.push(vec![span(rule, dim)]);
            }
        }
        out
    }
}
