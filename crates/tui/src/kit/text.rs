use crate::kit::style::Style;
use unicode_width::UnicodeWidthChar;

#[derive(Debug, Clone, PartialEq)]
pub struct Span {
    pub text: String,
    pub style: Style,
}

pub type Line = Vec<Span>;

pub fn span(text: impl Into<String>, style: Style) -> Span {
    Span {
        text: text.into(),
        style,
    }
}

pub fn width(s: &str) -> usize {
    s.chars().map(char_width).sum()
}

pub fn char_width(c: char) -> usize {
    if c == '\t' { 4 } else { c.width().unwrap_or(0) }
}

pub fn line_width(l: &Line) -> usize {
    l.iter().map(|s| width(&s.text)).sum()
}

pub fn clip(s: &str, max: usize, more: &str) -> String {
    if width(s) <= max {
        return s.to_string();
    }
    let room = max.saturating_sub(width(more));
    let mut out = String::new();
    let mut w = 0;
    for c in s.chars() {
        w += char_width(c);
        if w > room {
            break;
        }
        out.push(c);
    }
    out + more
}

pub fn clip_line(line: &Line, max: usize, more: &str) -> Line {
    if line_width(line) <= max {
        return line.clone();
    }
    let room = max.saturating_sub(width(more));
    let mut out: Line = vec![];
    let mut w = 0;
    for s in line {
        let mut text = String::new();
        for c in s.text.chars() {
            if w + char_width(c) > room {
                text.push_str(more);
                out.push(span(text, s.style));
                return out;
            }
            w += char_width(c);
            text.push(c);
        }
        out.push(span(text, s.style));
    }
    out
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct CopySpan {
    pub col: usize,
    pub text: String,
    pub soft: Option<String>,
}

impl CopySpan {
    pub fn end(&self) -> usize {
        self.col + width(&self.text)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum CopyRow {
    Text(CopySpan),
    Table {
        record: usize,
        cells: Vec<(usize, CopySpan)>,
    },
}

impl CopyRow {
    pub fn spans(&self) -> impl Iterator<Item = &CopySpan> {
        let (text, cells) = match self {
            Self::Text(text) => (Some(text), &[][..]),
            Self::Table { cells, .. } => (None, cells.as_slice()),
        };
        text.into_iter().chain(cells.iter().map(|(_, cell)| cell))
    }

    pub fn place(&mut self, inset: usize, width: usize) {
        let place = |span: &mut CopySpan| {
            if span.end() > width {
                span.text = clip(&span.text, width.saturating_sub(span.col), "");
            }
            span.col += inset;
        };
        match self {
            Self::Text(text) => place(text),
            Self::Table { cells, .. } => cells.iter_mut().for_each(|(_, cell)| place(cell)),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct MessageRow {
    pub line: Line,
    pub copy: Option<CopyRow>,
}

impl Default for MessageRow {
    fn default() -> Self {
        Self {
            line: vec![],
            copy: Some(CopyRow::Text(CopySpan::default())),
        }
    }
}

impl MessageRow {
    pub fn decoration(line: Line) -> Self {
        Self { line, copy: None }
    }
}

pub fn wrap(content: &Line, width: usize, first: &Line, rest: &Line) -> Vec<Line> {
    let mut out = vec![];
    wrap_rows(content, width, first, rest, |prefix, visible, _| {
        out.push(join(prefix, visible))
    });
    out
}

pub fn wrap_message(
    content: &Line,
    width: usize,
    first: &Line,
    rest: &Line,
    first_content: usize,
) -> Vec<MessageRow> {
    let mut out = vec![];
    wrap_rows(content, width, first, rest, |prefix, visible, skipped| {
        let mut col = line_width(prefix);
        let mut text = String::new();
        if out.is_empty() && first_content > 0 {
            col -= first_content;
            let mut at = 0;
            for c in prefix.iter().flat_map(|span| span.text.chars()) {
                if at >= col {
                    text.push(c);
                }
                at += char_width(c);
            }
        }
        text.extend(visible.iter().map(|&(c, _)| c));
        out.push(MessageRow {
            line: join(prefix, visible),
            copy: Some(CopyRow::Text(CopySpan {
                col,
                text,
                soft: skipped.map(|cells| cells.iter().map(|&(c, _)| c).collect()),
            })),
        });
    });
    out
}

fn wrap_rows(
    content: &Line,
    width: usize,
    first: &Line,
    rest: &Line,
    mut emit: impl FnMut(&Line, &[(char, Style)], Option<&[(char, Style)]>),
) {
    let cells: Vec<_> = content
        .iter()
        .flat_map(|s| {
            s.text
                .chars()
                .map(move |c| (if c == '\t' { ' ' } else { c }, s.style))
        })
        .collect();
    let rooms = [first, rest].map(|p| width.saturating_sub(line_width(p)).max(1));
    let (mut start, mut used, mut prefix, mut room) = (0, 0, first, rooms[0]);
    for (i, &(c, _)) in cells.iter().enumerate() {
        let w = char_width(c);
        if used + w > room && i > start {
            let split = match cells[start..i].iter().rposition(|(ch, _)| *ch == ' ') {
                Some(j) if j > 0 && c != ' ' => start + j + 1,
                _ => i,
            };
            let mut end = split;
            while end > start + 1 && cells[end - 1].0 == ' ' {
                end -= 1;
            }
            let skip = c == ' ' && split == i;
            let next = split + usize::from(skip);
            emit(prefix, &cells[start..end], Some(&cells[end..next]));
            (prefix, room, start) = (rest, rooms[1], next);
            used = cells[split..i].iter().map(|(ch, _)| char_width(*ch)).sum();
            if skip {
                continue;
            }
        }
        used += w;
    }
    emit(prefix, &cells[start..], None);
}

fn join(prefix: &Line, cells: &[(char, Style)]) -> Line {
    let mut line = prefix.clone();
    for (c, st) in cells {
        match line.last_mut() {
            Some(s) if s.style == *st => s.text.push(*c),
            _ => line.push(span(c.to_string(), *st)),
        }
    }
    line
}

pub(crate) fn clean(text: &str) -> String {
    text.chars()
        .filter(|c| *c == '\n' || *c == '\t' || !c.is_control())
        .map(|c| {
            if c == '\t' {
                "    ".to_string()
            } else {
                c.to_string()
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clip_line_keeps_styles() {
        let a = Style::default().bold();
        let b = Style::default();
        let line = vec![span("ab", a), span("cdef", b)];
        assert_eq!(clip_line(&line, 6, "…"), line);
        assert_eq!(clip_line(&line, 4, "…"), vec![span("ab", a), span("c…", b)]);
        assert_eq!(clip_line(&line, 2, "…"), vec![span("a…", a)]);
        assert_eq!(clip_line(&line, 0, "…"), vec![span("…", a)]);
    }

    fn plain(lines: &[Line]) -> Vec<String> {
        lines
            .iter()
            .map(|l| l.iter().map(|s| s.text.as_str()).collect())
            .collect()
    }

    #[test]
    fn wraps_words_with_prefixes() {
        let st = Style::default();
        let l = vec![span("hello big world", st)];
        let got = wrap(&l, 8, &vec![span("> ", st)], &vec![span("  ", st)]);
        assert_eq!(plain(&got), ["> hello", "  big", "  world"]);
        let long = vec![span("abcdefghij", st)];
        assert_eq!(
            plain(&wrap(&long, 4, &vec![], &vec![])),
            ["abcd", "efgh", "ij"]
        );
        assert_eq!(clip("abcdef", 4, "…"), "abc…");
    }
}
