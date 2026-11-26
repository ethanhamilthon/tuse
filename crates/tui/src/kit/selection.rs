use crate::kit::style::Style;
use crate::kit::text::{CopyRow, CopySpan, Line, char_width, span};

#[derive(Clone)]
pub struct HitRow {
    pub id: u64,
    pub index: usize,
    pub copy: CopyRow,
}

impl HitRow {
    pub fn start(&self) -> usize {
        self.copy
            .spans()
            .find(|span| !span.text.is_empty())
            .map_or(0, |span| span.col)
    }

    pub fn end(&self) -> usize {
        self.copy
            .spans()
            .filter(|span| !span.text.is_empty())
            .map(CopySpan::end)
            .max()
            .unwrap_or(0)
    }

    pub fn contains(&self, col: usize) -> bool {
        self.copy
            .spans()
            .any(|span| !span.text.is_empty() && (span.col..span.end()).contains(&col))
            || self.is_text() && col == self.end()
    }

    pub fn is_text(&self) -> bool {
        self.start() < self.end()
    }

    pub fn at(&self, col: usize) -> Point {
        (self.index, col.clamp(self.start(), self.end()))
    }
}

type Point = (usize, usize);

#[derive(Clone, Copy)]
pub enum Gesture {
    Input(usize),
    Chat { id: u64, anchor: Point },
}

#[derive(Clone, Copy)]
pub struct MessageSelection {
    pub id: u64,
    pub anchor: Point,
    pub end: Point,
}

#[derive(Default)]
pub struct Selection {
    pub gesture: Option<Gesture>,
    pub message: Option<MessageSelection>,
}

impl MessageSelection {
    fn ranges(
        self,
        rows: &[Option<HitRow>],
    ) -> impl Iterator<Item = (usize, &HitRow, usize, usize)> {
        let (first, last) = (self.anchor.min(self.end), self.anchor.max(self.end));
        rows.iter().enumerate().filter_map(move |(y, row)| {
            let r = row.as_ref()?;
            if self.id != r.id || r.index < first.0 || r.index > last.0 || first == last {
                return None;
            }
            let start = if r.index == first.0 {
                r.at(first.1).1
            } else {
                r.start()
            };
            let end = if r.index == last.0 {
                r.at(last.1).1
            } else {
                r.end()
            };
            Some((y, r, start, end))
        })
    }

    pub fn render(self, rows: &[Option<HitRow>], screen: &mut [Line], style: Style) {
        for (y, row, start, end) in self.ranges(rows) {
            for span in row.copy.spans() {
                highlight(
                    &mut screen[y],
                    start.max(span.col),
                    end.min(span.end()),
                    style,
                );
            }
        }
    }

    pub fn text(self, rows: &[Option<HitRow>]) -> Option<String> {
        let mut text = String::new();
        let mut ranges = self.ranges(rows).peekable();
        while let Some((_, row, start, end)) = ranges.next() {
            match &row.copy {
                CopyRow::Text(span) => {
                    copy_span(span, start, end, &mut text);
                    if ranges.peek().is_some() {
                        text.push_str(span.soft.as_deref().unwrap_or("\n"));
                    }
                }
                CopyRow::Table { record, cells } => {
                    let mut fields = std::collections::BTreeMap::new();
                    copy_cells(cells, start, end, &mut fields);
                    while let Some((_, next, _, _)) = ranges.peek()
                        && matches!(&next.copy, CopyRow::Table { record: r, .. } if r == record)
                    {
                        let Some((_, next, start, end)) = ranges.next() else {
                            break;
                        };
                        if let CopyRow::Table { cells, .. } = &next.copy {
                            copy_cells(cells, start, end, &mut fields);
                        }
                    }
                    for (index, (field, _)) in fields.into_values().enumerate() {
                        if index > 0 {
                            text.push_str(" | ");
                        }
                        text.push_str(&field);
                    }
                    if ranges.peek().is_some() {
                        text.push('\n');
                    }
                }
            }
        }
        (!text.is_empty()).then_some(text)
    }
}

fn copy_cells<'a>(
    cells: &'a [(usize, CopySpan)],
    start: usize,
    end: usize,
    fields: &mut std::collections::BTreeMap<usize, (String, Option<&'a str>)>,
) {
    for (column, span) in cells {
        if start >= end || start >= span.end() || end <= span.col {
            continue;
        }
        let (field, soft) = fields.entry(*column).or_default();
        if let Some(separator) = soft.take() {
            field.push_str(separator);
        }
        copy_span(span, start, end, field);
        *soft = span.soft.as_deref();
    }
}

fn copy_span(span: &CopySpan, start: usize, end: usize, out: &mut String) {
    if start >= end {
        return;
    }
    let mut col = span.col;
    let mut inside = false;
    for c in span.text.chars() {
        let w = char_width(c);
        if w != 0 {
            inside = col < end && col + w > start;
        }
        if inside {
            out.push(c);
        }
        col += w;
    }
}

pub fn highlight(line: &mut Line, start: usize, end: usize, style: Style) {
    if start >= end {
        return;
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
}
