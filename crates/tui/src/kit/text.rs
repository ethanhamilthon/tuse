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

pub fn wrap(content: &Line, width: usize, first: &Line, rest: &Line) -> Vec<Line> {
    let cells: Vec<(char, Style)> = content
        .iter()
        .flat_map(|s| {
            s.text
                .chars()
                .map(move |c| (if c == '\t' { ' ' } else { c }, s.style))
        })
        .collect();
    let mut out: Vec<Line> = vec![];
    let mut cur: Vec<(char, Style)> = vec![];
    let mut cur_w = 0;
    let mut prefix = first;
    let rooms = [first, rest].map(|p| width.saturating_sub(line_width(p)).max(1));
    let mut room = rooms[0];
    for cell in cells {
        let w = char_width(cell.0);
        if cur_w + w > room && !cur.is_empty() {
            let carry = match cur.iter().rposition(|(c, _)| *c == ' ') {
                Some(i) if i > 0 && cell.0 != ' ' => cur.split_off(i + 1),
                _ => vec![],
            };
            while cur.last().is_some_and(|(c, _)| *c == ' ') && cur.len() > 1 {
                cur.pop();
            }
            out.push(join(prefix, &cur));
            prefix = rest;
            room = rooms[1];
            cur = carry;
            cur_w = cur.iter().map(|(c, _)| char_width(*c)).sum();
            if cell.0 == ' ' && cur.is_empty() {
                continue;
            }
        }
        cur.push(cell);
        cur_w += w;
    }
    out.push(join(prefix, &cur));
    out
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
