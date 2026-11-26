use crate::kit::style::{Palette, RULE_CHAR, Style, TABLE_SEP};
use crate::kit::text::{CopyRow, Line, MessageRow, line_width, span, width, wrap_message};
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
    pub fn prefixed(
        self,
        available: usize,
        p: &Palette,
        first: Line,
        rest: Line,
    ) -> Vec<MessageRow> {
        let room = available.saturating_sub(line_width(&first).max(line_width(&rest)));
        self.render(room, p)
            .into_iter()
            .enumerate()
            .map(|(i, mut row)| {
                let mut prefix = if i == 0 { first.clone() } else { rest.clone() };
                if let Some(copy) = &mut row.copy {
                    copy.place(line_width(&prefix), room);
                }
                prefix.append(&mut row.line);
                row.line = prefix;
                row
            })
            .collect()
    }
    fn render(self, available: usize, p: &Palette) -> Vec<MessageRow> {
        let dim = Style::fg(p.muted);
        let mut out = vec![];
        let Some(widths) = self.widths(available) else {
            if let Some((headers, rows)) = self.rows.split_first() {
                for (r, row) in rows.iter().enumerate() {
                    if r > 0 {
                        out.push(MessageRow::decoration(vec![span(
                            RULE_CHAR.repeat(available),
                            dim,
                        )]));
                    }
                    for (i, cell) in row.iter().enumerate() {
                        let key: String = headers
                            .get(i)
                            .into_iter()
                            .flatten()
                            .map(|s| s.text.as_str())
                            .collect();
                        let label = crate::kit::text::clip(
                            &format!("{key}: "),
                            available.saturating_sub(2),
                            "",
                        );
                        let pad = vec![span(" ".repeat(width(&label)), dim)];
                        let first = vec![span(label, dim)];
                        out.extend(
                            wrap_message(cell, available, &first, &pad, 0)
                                .into_iter()
                                .map(|mut row| {
                                    if let Some(CopyRow::Text(copy)) = row.copy.take() {
                                        row.copy = Some(CopyRow::Table {
                                            record: r,
                                            cells: vec![(i, copy)],
                                        });
                                    }
                                    row
                                }),
                        );
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
                    wrap_message(&cell, w, &vec![], &vec![], 0)
                })
                .collect();
            for j in 0..cells.iter().map(Vec::len).max().unwrap_or(0) {
                let mut line = vec![];
                let mut copy_cells = vec![];
                for (i, lines) in cells.iter().enumerate() {
                    if i > 0 {
                        line.push(span(TABLE_SEP, dim));
                    }
                    let cell = lines.get(j);
                    let space = widths[i].saturating_sub(cell.map_or(0, |r| line_width(&r.line)));
                    let left = match self.align.get(i) {
                        Some(Alignment::Right) => space,
                        Some(Alignment::Center) => space / 2,
                        _ => 0,
                    };
                    if left > 0 {
                        line.push(span(" ".repeat(left), Style::default()));
                    }
                    if let Some(cell) = cell {
                        if let Some(CopyRow::Text(mut copy)) = cell.copy.clone() {
                            copy.col += line_width(&line);
                            copy_cells.push((i, copy));
                        }
                        line.extend(cell.line.iter().cloned());
                    }
                    if i + 1 < cells.len() && space > left {
                        line.push(span(" ".repeat(space - left), Style::default()));
                    }
                }
                out.push(MessageRow {
                    line,
                    copy: Some(CopyRow::Table {
                        record: r,
                        cells: copy_cells,
                    }),
                });
            }
            if r == 0 {
                out.push(MessageRow::decoration(vec![span(
                    RULE_CHAR.repeat(total),
                    dim,
                )]));
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
        let p = Palette::new(&Default::default());
        for (n, w, records) in [(3, 35, false), (15, 80, true)] {
            let lines: Vec<_> = make(n)
                .render(w, &p)
                .into_iter()
                .map(|row| row.line)
                .collect();
            assert!(lines.iter().all(|l| line_width(l) <= w));
            assert_eq!(
                lines.iter().flatten().any(|s| s.text.contains("Name:")),
                records
            );
        }
    }
}
