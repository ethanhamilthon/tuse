use crate::style::Palette;
use crate::text::Line;

pub struct Prefix {
    pub entry: usize,
    width: usize,
    len: usize,
    lines: Vec<Line>,
}

pub fn split(text: &str) -> usize {
    let (mut fence, mut blank, mut at, mut pos) = (false, false, 0, 0);
    for line in text.split_inclusive('\n') {
        let body = line.trim_end_matches(['\n', '\r']);
        let t = body.trim_start();
        let fenced = t.starts_with("```") || t.starts_with("~~~");
        if !fence && blank && !t.is_empty() && starts_block(body, line.ends_with('\n')) {
            at = pos;
        }
        if fenced {
            fence = !fence;
        }
        blank = !fence && t.is_empty();
        pos += line.len();
    }
    at
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

pub fn render(
    cache: &mut Option<Prefix>,
    entry: usize,
    text: &str,
    width: usize,
    p: &Palette,
) -> Vec<Line> {
    let at = split(text);
    if cache
        .as_ref()
        .is_none_or(|c| c.entry != entry || c.width != width || c.len > at)
    {
        *cache = Some(Prefix {
            entry,
            width,
            len: 0,
            lines: vec![],
        });
    }
    let c = cache.as_mut().unwrap();
    if c.len < at {
        let more = crate::markdown::render(&text[c.len..at], width, p);
        join(&mut c.lines, more);
        c.len = at;
    }
    let mut lines = c.lines.clone();
    join(&mut lines, crate::markdown::render(&text[at..], width, p));
    lines
}

fn join(lines: &mut Vec<Line>, more: Vec<Line>) {
    if !lines.is_empty() && !more.is_empty() {
        lines.push(vec![]);
    }
    lines.extend(more);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_only_between_independent_blocks() {
        for (text, at) in [
            ("one\n\ntwo", 5),
            ("one\n\ntwo\n\nthr", 10),
            ("```\na\n\nb\n```\n\nc", 14),
            ("```\na\n\nb", 0),
            ("- a\n\n- b", 0),
            ("1. a\n\n2. b", 0),
            ("- a\n\n  more", 0),
            ("one\n\n", 0),
        ] {
            assert_eq!(split(text), at, "{text:?}");
        }
    }

    #[test]
    fn streaming_renders_like_the_whole_text() {
        let p = Palette::new(&Default::default(), &Default::default());
        let text = &"# T\n\npara **b**\n\n| a | b |\n|---|---|\n| 1 | 2 |\n\n```py\nx = 1\n\ny\n```\n\n- i\n- j\n\n> q\n\n1. a\n\n2. b\n\n---\n\nend\n\n".repeat(3);
        let mut cache = None;
        for n in (1..=text.len()).filter(|n| text.is_char_boundary(*n)) {
            let got = render(&mut cache, 0, &text[..n], 30, &p);
            let want = crate::markdown::render(&text[..n], 30, &p);
            let s = |l: &Vec<Line>| {
                l.iter()
                    .map(|l| l.iter().map(|s| s.text.as_str()).collect::<String>())
                    .collect::<Vec<_>>()
            };
            assert_eq!(s(&got), s(&want), "{n}: {:?}", &text[..n]);
            assert_eq!(got, want, "{n}");
        }
    }
}
