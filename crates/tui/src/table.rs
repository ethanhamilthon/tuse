use crate::style::{Palette, RULE_CHAR, Style, TABLE_SEP};
use crate::text::{Line, line_width, span, width, wrap};
use pulldown_cmark::Alignment;

#[derive(Default)]
pub struct Table {
    pub rows: Vec<Vec<Line>>,
    pub row: Vec<Line>,
    pub align: Vec<Alignment>,
}
impl Table {
    fn widths(&self, available: usize) -> Option<Vec<usize>> {
        let n = self.rows.iter().map(Vec::len).max().unwrap_or(0);
        let room = available.checked_sub(width(TABLE_SEP) * n.saturating_sub(1))?;
        let mut natural = vec![0; n];
        let mut floor = vec![0; n];
        for row in &self.rows {
            for (i, cell) in row.iter().enumerate() {
                natural[i] = natural[i].max(line_width(cell));
                let text: String = cell.iter().map(|s| s.text.as_str()).collect();
                floor[i] = floor[i].max(
                    text.split_whitespace()
                        .map(width)
                        .max()
                        .unwrap_or(0)
                        .clamp(3, 12),
                );
            }
        }
        if natural.iter().sum::<usize>() <= room {
            return Some(natural);
        }
        for (f, w) in floor.iter_mut().zip(&natural) {
            *f = (*f).min(*w);
        }
        if floor.iter().sum::<usize>() > room {
            return None;
        }
        let mut left = room - floor.iter().sum::<usize>();
        while left > 0 {
            let i = (0..n)
                .filter(|&i| floor[i] < natural[i])
                .min_by_key(|&i| floor[i])?;
            floor[i] += 1;
            left -= 1;
        }
        Some(floor)
    }
    pub fn render(self, available: usize, p: &Palette) -> Vec<Line> {
        let dim = Style::fg(p.dim);
        let mut out = vec![];
        let Some(widths) = self.widths(available) else {
            if let Some((headers, rows)) = self.rows.split_first() {
                for (r, row) in rows.iter().enumerate() {
                    if r > 0 {
                        out.push(vec![span(RULE_CHAR.repeat(available), dim)]);
                    }
                    for (i, cell) in row.iter().enumerate() {
                        let key: String = headers
                            .get(i)
                            .into_iter()
                            .flatten()
                            .map(|s| s.text.as_str())
                            .collect();
                        let mut line = vec![span(format!("{key}: "), dim)];
                        line.extend(cell.clone());
                        let pad = vec![span(
                            " ".repeat((width(&key) + 2).min(available.saturating_sub(2))),
                            dim,
                        )];
                        out.extend(wrap(&line, available, &vec![], &pad));
                    }
                }
            }
            return out;
        };
        let total =
            widths.iter().sum::<usize>() + width(TABLE_SEP) * widths.len().saturating_sub(1);
        for (r, row) in self.rows.iter().enumerate() {
            let cells: Vec<_> = widths
                .iter()
                .enumerate()
                .map(|(i, &w)| {
                    let mut cell = row.get(i).cloned().unwrap_or_default();
                    if r == 0 {
                        for s in &mut cell {
                            s.style = s.style.bold();
                        }
                    }
                    wrap(&cell, w, &vec![], &vec![])
                })
                .collect();
            for j in 0..cells.iter().map(Vec::len).max().unwrap_or(0) {
                let mut line = vec![];
                for (i, lines) in cells.iter().enumerate() {
                    if i > 0 {
                        line.push(span(TABLE_SEP, dim));
                    }
                    let cell = lines.get(j).cloned().unwrap_or_default();
                    let space = widths[i].saturating_sub(line_width(&cell));
                    let left = match self.align.get(i) {
                        Some(Alignment::Right) => space,
                        Some(Alignment::Center) => space / 2,
                        _ => 0,
                    };
                    if left > 0 {
                        line.push(span(" ".repeat(left), Style::default()));
                    }
                    line.extend(cell);
                    if i + 1 < cells.len() && space > left {
                        line.push(span(" ".repeat(space - left), Style::default()));
                    }
                }
                out.push(line);
            }
            if r == 0 {
                out.push(vec![span(RULE_CHAR.repeat(total), dim)]);
            }
        }
        out
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn wraps_and_switches_to_records() {
        let make = |n| {
            let cell = |s: &str| vec![span(s, Style::default())];
            Table {
                rows: vec![
                    vec![cell("Name"); n],
                    vec![cell("Testing 世界 with words"); n],
                ],
                ..Default::default()
            }
        };
        let p = Palette::new(&Default::default(), &Default::default());
        for (n, w, records) in [(3, 35, false), (15, 80, true)] {
            let lines = make(n).render(w, &p);
            assert!(lines.iter().all(|l| line_width(l) <= w));
            assert_eq!(
                lines.iter().flatten().any(|s| s.text.contains("Name:")),
                records
            );
        }
    }
}
