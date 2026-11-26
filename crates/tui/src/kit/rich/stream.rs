use crate::kit::rich::render::{chat_inset, chat_marker, fit_chat};
use crate::kit::style::{Palette, REASONING_PREFIX, Style};
use crate::kit::text::{Line, MessageRow};
use std::ops::Range;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Cut {
    pub at: usize,
    pub gap: bool,
    pub fence: Option<Range<usize>>,
}

pub struct Prefix<T> {
    width: usize,
    cut: Cut,
    lines: Vec<T>,
}

fn incremental<T: Clone + Default>(
    cache: &mut Option<Prefix<T>>,
    width: usize,
    start: Cut,
    cut: Cut,
    render: impl Fn(&Cut, Option<usize>) -> Vec<T>,
) -> Vec<T> {
    if cache.as_ref().is_none_or(|c| {
        c.width != width
            || c.cut.at < start.at
            || c.cut.at > cut.at
            || (c.cut.at == cut.at && c.cut != cut)
    }) {
        *cache = Some(Prefix {
            width,
            cut: start,
            lines: vec![],
        });
    }
    let c = cache.as_mut().unwrap();
    if c.cut.at < cut.at {
        let more = render(&c.cut, Some(cut.at));
        join(&mut c.lines, more, c.cut.gap);
        c.cut = cut;
    }
    let mut lines = c.lines.clone();
    join(&mut lines, render(&c.cut, None), c.cut.gap);
    lines
}

fn join<T: Default>(lines: &mut Vec<T>, more: Vec<T>, gap: bool) {
    if gap && !lines.is_empty() && !more.is_empty() {
        lines.push(T::default());
    }
    lines.extend(more);
}

pub fn markdown(
    cache: &mut Option<Prefix<MessageRow>>,
    text: &str,
    width: usize,
    p: &Palette,
) -> Vec<MessageRow> {
    let render = |s: &str| crate::kit::rich::markdown::rows(s, width, p, true);
    incremental(cache, width, Cut::default(), split(text), |c, end| {
        let body = &text[c.at..end.unwrap_or(text.len())];
        let Some(open) = &c.fence else {
            return render(body);
        };
        let open = &text[open.clone()];
        let mut lines = render(&format!("{open}{body}"));
        lines.drain(..render(open).len().min(lines.len()));
        lines
    })
}

pub fn reasoning(
    cache: &mut Option<Prefix<Line>>,
    text: &str,
    width: usize,
    p: &Palette,
) -> Vec<Line> {
    let base = text.len() - text.trim_start().len();
    let st = Style::fg(p.muted).italic();
    let pad = " ".repeat(chat_inset(width));
    let marker = chat_marker(REASONING_PREFIX, width);
    let start = Cut {
        at: base,
        ..Cut::default()
    };
    incremental(
        cache,
        width,
        start,
        line_cut(text, base, false),
        |c, end| {
            let first = if c.at == base { &marker } else { &pad };
            match body(text, c.at, end) {
                Some(b) => fit_chat(
                    crate::kit::rich::render::plain(b, width, st, first, &pad),
                    width,
                ),
                None => vec![],
            }
        },
    )
}

pub fn tool(cache: &mut Option<Prefix<Line>>, input: &str, width: usize, p: &Palette) -> Vec<Line> {
    let base = input.len() - input.trim_start().len();
    let head = Style::fg(p.fg);
    let start = Cut {
        at: base,
        ..Cut::default()
    };
    let lines = incremental(
        cache,
        width,
        start,
        line_cut(input, base, true),
        |c, end| {
            let first = c.at == base;
            match body(input, c.at, end) {
                Some(b) => crate::kit::rich::tool_view::source(b, first, width, head, false, p),
                None => vec![],
            }
        },
    );
    fit_chat(
        crate::kit::rich::tool_view::finish(lines, None, &[], width, p),
        width,
    )
}

fn body(text: &str, at: usize, end: Option<usize>) -> Option<&str> {
    match end {
        Some(end) => Some(&text[at..end - 1]),
        None => Some(text[at..].trim_end()).filter(|b| !b.is_empty()),
    }
}

fn line_cut(text: &str, base: usize, lua: bool) -> Cut {
    let mut at = cuts(text, base, lua, true);
    if lua {
        let other = cuts(text, base, true, false);
        at.retain(|p| other.binary_search(p).is_ok());
    }
    Cut {
        at: at.last().copied().unwrap_or(base),
        ..Cut::default()
    }
}

fn cuts(text: &str, base: usize, lua: bool, escaped_newline: bool) -> Vec<usize> {
    let b = text.as_bytes();
    let last = b
        .iter()
        .rposition(|c| !c.is_ascii_whitespace())
        .map_or(0, |i| i + 1);
    let mut out = vec![];
    let (mut i, mut filled, mut close) = (base, false, None::<usize>);
    while i < b.len() {
        let c = b[i];
        i += 1;
        if let Some(eq) = close {
            if c == b']'
                && b[i..].iter().take(eq).all(|c| *c == b'=')
                && b.get(i + eq) == Some(&b']')
            {
                close = None;
                filled = true;
                i += eq + 1;
            }
            continue;
        }
        match c {
            b'\n' => {
                if filled && i < last {
                    out.push(i);
                }
                filled = false;
                continue;
            }
            b'"' | b'\'' if lua => {
                while i < b.len() {
                    let cur = b[i];
                    i += 1;
                    if cur == b'\\' && (escaped_newline || b.get(i) != Some(&b'\n')) {
                        i += 1;
                    } else if cur == c {
                        break;
                    } else if cur == b'\n' {
                        i -= 1;
                        break;
                    }
                }
            }
            b'-' if lua && b.get(i) == Some(&b'-') => match long(b, i + 1) {
                Some(eq) => {
                    close = Some(eq);
                    i += eq + 3;
                }
                None => {
                    while i < b.len() && b[i] != b'\n' {
                        i += 1;
                    }
                }
            },
            b'[' if lua => {
                if let Some(eq) = long(b, i - 1) {
                    close = Some(eq);
                    i += eq + 1;
                }
            }
            _ => {}
        }
        filled |= !c.is_ascii_whitespace();
    }
    out
}

#[derive(Clone, Copy, PartialEq)]
enum List {
    No,
    Unknown,
    In { marker: u8, next: u64, width: usize },
}

struct Fence {
    ch: u8,
    n: usize,
    open: Range<usize>,
    split: bool,
    lua: bool,
    filled: bool,
}

pub fn split(text: &str) -> Cut {
    let mut cut = Cut::default();
    let mut fence: Option<Fence> = None;
    let (mut blank, mut list, mut raw, mut pos) = (true, List::No, false, 0);
    for line in text.split_inclusive('\n') {
        let whole = line.ends_with('\n');
        let body = line.trim_end_matches(['\n', '\r']);
        let t = body.trim_start();
        let indent = body.len() - t.len();
        let at = pos;
        pos += line.len();
        if let Some(f) = &mut fence {
            if indent <= 3 && closes(t, f.ch, f.n) {
                fence = None;
                blank = false;
                continue;
            }
            if f.split && f.filled && !t.is_empty() {
                cut = Cut {
                    at,
                    gap: false,
                    fence: Some(f.open.clone()),
                };
            }
            f.filled = !t.is_empty();
            f.split &= !(f.lua && (t.contains("[[") || t.contains("[=")));
            continue;
        }
        if t.is_empty() {
            blank = true;
            continue;
        }
        raw |= indent <= 3 && t.starts_with('<');
        if let Some((marker, num, width)) = item(body).filter(|_| whole && !raw) {
            let gap = match list {
                List::In {
                    marker: m, next, ..
                } if m == marker => (num == next).then_some(false),
                List::In { .. } => Some(true),
                List::No if blank => Some(true),
                _ => None,
            };
            list = match gap {
                Some(gap) => {
                    cut = Cut {
                        at,
                        gap,
                        fence: None,
                    };
                    let next = if matches!(marker, b'.' | b')') {
                        num + 1
                    } else {
                        0
                    };
                    List::In {
                        marker,
                        next,
                        width,
                    }
                }
                None => List::Unknown,
            };
            blank = false;
            continue;
        }
        if blank && starts_block(body, whole) {
            cut = Cut {
                at,
                gap: true,
                fence: None,
            };
        }
        let opener = (indent <= 3 || list != List::No)
            .then(|| opens(t))
            .flatten();
        list = match list {
            List::In { width, .. } if indent >= width => list,
            _ if t.starts_with(['-', '*', '+']) || t.starts_with(|c: char| c.is_ascii_digit()) => {
                List::Unknown
            }
            _ if blank && indent == 0 => List::No,
            List::No => List::No,
            _ => List::Unknown,
        };
        if let Some((ch, n)) = opener {
            let lang = t[n..].split_whitespace().next().unwrap_or("");
            fence = Some(Fence {
                ch,
                n,
                open: at..pos,
                split: indent == 0 && whole && !raw,
                lua: lang.eq_ignore_ascii_case("lua"),
                filled: true,
            });
        }
        blank = false;
    }
    cut
}

fn opens(t: &str) -> Option<(u8, usize)> {
    let ch = *t.as_bytes().first()?;
    if ch != b'`' && ch != b'~' {
        return None;
    }
    let n = t.bytes().take_while(|c| *c == ch).count();
    (n >= 3 && !(ch == b'`' && t[n..].contains('`'))).then_some((ch, n))
}

fn closes(t: &str, ch: u8, n: usize) -> bool {
    let run = t.bytes().take_while(|c| *c == ch).count();
    run >= n && t[run..].trim().is_empty()
}

fn item(body: &str) -> Option<(u8, u64, usize)> {
    let b = body.as_bytes();
    let (marker, num, len) = match *b.first()? {
        c @ (b'-' | b'*' | b'+') => (c, 0, 1),
        c if c.is_ascii_digit() => {
            let d = b.iter().take_while(|c| c.is_ascii_digit()).count();
            let delim = *b.get(d)?;
            if d > 9 || !matches!(delim, b'.' | b')') {
                return None;
            }
            (delim, body[..d].parse().ok()?, d + 1)
        }
        _ => return None,
    };
    let rest = &body[len..];
    let spaces = rest.bytes().take_while(|c| *c == b' ').count();
    let content = &rest[spaces..];
    if spaces == 0 || content.trim().is_empty() || content.starts_with('\t') {
        return None;
    }
    if marker != b'+'
        && len == 1
        && body.bytes().all(|c| c == marker || c == b' ' || c == b'\t')
        && body.bytes().filter(|c| *c == marker).count() >= 3
    {
        return None;
    }
    let width = len + if spaces >= 5 { 1 } else { spaces };
    Some((marker, num, width))
}

fn starts_block(line: &str, whole: bool) -> bool {
    let Some(c) = line.chars().next() else {
        return false;
    };
    if !whole && (c.is_ascii_digit() || matches!(c, '-' | '*' | '+')) {
        return false;
    }
    let item = matches!(c, '-' | '*' | '+') && line[1..].starts_with([' ', '\t'])
        || c.is_ascii_digit() && {
            let rest = line.trim_start_matches(|c: char| c.is_ascii_digit());
            rest.starts_with(". ") || rest.starts_with(") ")
        };
    !c.is_whitespace() && !item
}

fn long(b: &[u8], at: usize) -> Option<usize> {
    if b.get(at) != Some(&b'[') {
        return None;
    }
    let eq = b[at + 1..].iter().take_while(|c| **c == b'=').count();
    (b.get(at + 1 + eq) == Some(&b'[')).then_some(eq)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_only_where_the_rest_renders_independently() {
        for (text, at, gap) in [
            ("one\n\ntwo", 5, true),
            ("one\n\ntwo\n\nthr", 10, true),
            ("```\na\n\nb\n```\n\nc", 14, true),
            ("```\na\n\nb", 4, false),
            ("~~~lua\nx = [[\ny\n", 7, false),
            ("- a\n\n- b\n", 5, false),
            ("- a\n* b\n", 4, true),
            ("1. a\n2. b\n3. c", 5, false),
            ("1. a\n1. b\n1. c", 0, true),
            ("- a\nlazy\n- b\n", 0, true),
            ("- a\n\n  more", 0, true),
            ("<div>\n- a\n- b\n", 0, true),
            ("para\n- a\n- b\n", 0, true),
            ("one\n\n", 0, true),
        ] {
            let c = split(text);
            assert_eq!((c.at, c.gap), (at, gap), "{text:?}");
        }
    }

    fn check<T: std::fmt::Debug + PartialEq>(
        text: &str,
        stream: impl Fn(&mut Option<Prefix<T>>, &str) -> Vec<T>,
        full: impl Fn(&str) -> Vec<T>,
    ) {
        let ends: Vec<usize> = (1..=text.len())
            .filter(|n| text.is_char_boundary(*n))
            .collect();
        for step in [1, 13] {
            let mut cache = None;
            for &n in ends.iter().step_by(step) {
                let (got, want) = (stream(&mut cache, &text[..n]), full(&text[..n]));
                assert_eq!(got, want, "{n}: {:?}", &text[..n]);
            }
        }
    }

    #[test]
    fn streaming_renders_like_the_whole_text() {
        let p = Palette::new(&Default::default());
        let md = "# T\n\npara **b**\n\n| a | b |\n|---|---|\n| 1 | 2 |\n\n```py\nx = 1\n\ny\n```\n\n\
            - i\n- j\n  - nested\n\n  more\n- k\n* other\n\n> q\n\n1. a\n2. b\n\n3. c\n9) d\n1. x\n1. y\n\n\
            - - -\n\n---\n\n```lua\nlocal s = [[\nlong\n]]\nprint(s)\n```\n\n- item\n  ```\n  code\n  ```\n- next\n\n\
            <div>\n\n- raw\n```\nin html\n</div>\n\nend\n\n";
        check(
            &md.repeat(2),
            |c, t| markdown(c, t, 30, &p),
            |t| crate::kit::rich::markdown::rows(t, 30, &p, true),
        );
        let thought =
            "  First thought that is long enough to wrap around.\n\nSecond\n  \nthird line\n\n";
        let full = |t: &str| {
            crate::kit::rich::render::render(&crate::kit::entry::Entry::Reasoning(t.into()), 24, &p)
        };
        check(
            &thought.repeat(2),
            |c, t| {
                if t.trim().is_empty() {
                    vec![]
                } else {
                    reasoning(c, t, 24, &p)
                }
            },
            full,
        );
        let lua = "\n  local x = 1 -- c\nwrite('a.md', [==[\n# h\n\nbody ]] still\n]==])\ns = \"q\\\n[[\" .. '[['\n--[[ c\nc ]] y = 2\n\n\nfor i = 1, 3 do\n  print(i)\nend\n  \n";
        let full = |t: &str| crate::kit::rich::tool_view::tool(t, None, None, &[], 24, &p);
        check(&lua.repeat(2), |c, t| tool(c, t, 24, &p), full);
    }
}
