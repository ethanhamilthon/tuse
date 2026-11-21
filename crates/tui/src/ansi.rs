//! Text with ANSI SGR styling (`draw_ui`): colors map to theme tokens, every other escape is dropped.
use crate::links;
use crate::style::{Palette, Style};
use crate::text::{Line, span, wrap};
use crossterm::style::Color;

/// Renders `text` to wrapped lines. `base` is the style of unstyled text; SGR state carries over
/// lines like in a terminal. Bare URLs and OSC 8 hyperlinks become clickable.
pub fn lines(text: &str, width: usize, base: Style, p: &Palette) -> Vec<Line> {
    let mut st = base;
    let mut link = None;
    let mut rows: Vec<Line> = vec![vec![]];
    let mut buf = String::new();
    let mut chars = text.chars().peekable();
    let flush = |buf: &mut String, st: Style, link: Option<u32>, rows: &mut Vec<Line>| {
        if buf.is_empty() {
            return;
        }
        let cur = rows.last_mut().unwrap();
        let text = std::mem::take(buf);
        if link.is_some() {
            cur.push(span(
                text,
                Style {
                    link,
                    underline: true,
                    ..st
                },
            ));
            return;
        }
        cur.extend(links::spans(&text, st, p));
    };
    while let Some(c) = chars.next() {
        match c {
            '\u{1b}' => {
                flush(&mut buf, st, link, &mut rows);
                match chars.peek() {
                    Some('[') => {
                        chars.next();
                        let mut params = String::new();
                        while let Some(&n) = chars.peek() {
                            chars.next();
                            if ('\u{40}'..='\u{7e}').contains(&n) {
                                if n == 'm' {
                                    sgr(&params, &mut st, base, p);
                                }
                                break;
                            }
                            params.push(n);
                        }
                    }
                    Some(']') => {
                        chars.next();
                        let mut body = String::new();
                        while let Some(n) = chars.next() {
                            if n == '\u{7}' || n == '\u{9c}' {
                                break;
                            }
                            if n == '\u{1b}' {
                                chars.next_if_eq(&'\\');
                                break;
                            }
                            body.push(n);
                        }
                        if let Some(uri) = body.strip_prefix("8;").and_then(|b| b.split_once(';')) {
                            link = (links::openable(uri.1)).then(|| links::intern(uri.1));
                        }
                    }
                    Some(&n) => {
                        chars.next();
                        // `ESC ( B` and friends: intermediate bytes, then one final byte.
                        if ('\u{20}'..='\u{2f}').contains(&n) {
                            chars.next();
                        }
                    }
                    None => {}
                }
            }
            '\n' => {
                flush(&mut buf, st, link, &mut rows);
                rows.push(vec![]);
            }
            '\t' => buf.push_str("    "),
            c if c.is_control() => {}
            c => buf.push(c),
        }
    }
    flush(&mut buf, st, link, &mut rows);
    rows.iter()
        .flat_map(|l| wrap(l, width, &vec![], &vec![]))
        .collect()
}

fn theme(n: u16, p: &Palette) -> Option<Color> {
    match n {
        0 => p.muted,
        1 | 9 => p.err,
        2 | 10 => p.ok,
        3 | 5 | 11 | 13 => p.accent,
        4 | 6 | 12 | 14 => p.info,
        8 => p.dim,
        _ => p.fg,
    }
}

fn sgr(params: &str, st: &mut Style, base: Style, p: &Palette) {
    let nums: Vec<u16> = if params.is_empty() {
        vec![0]
    } else {
        params
            .split([';', ':'])
            .map(|s| s.parse().unwrap_or(0))
            .collect()
    };
    let mut i = 0;
    while i < nums.len() {
        let n = nums[i];
        i += 1;
        match n {
            0 => *st = base,
            1 => st.bold = true,
            2 => st.dim = true,
            3 => st.italic = true,
            4 => st.underline = true,
            22 => (st.bold, st.dim) = (false, false),
            23 => st.italic = false,
            24 => st.underline = false,
            30..=37 => st.fg = theme(n - 30, p),
            90..=97 => st.fg = theme(n - 90 + 8, p),
            40..=47 => st.bg = theme(n - 40, p),
            100..=107 => st.bg = theme(n - 100 + 8, p),
            39 => st.fg = base.fg,
            49 => st.bg = base.bg,
            38 | 48 => {
                let color = match nums.get(i) {
                    Some(5) if i + 1 < nums.len() => {
                        i += 2;
                        Some(Color::AnsiValue(nums[i - 1].min(255) as u8))
                    }
                    Some(2) if i + 3 < nums.len() => {
                        i += 4;
                        Some(Color::Rgb {
                            r: nums[i - 3].min(255) as u8,
                            g: nums[i - 2].min(255) as u8,
                            b: nums[i - 1].min(255) as u8,
                        })
                    }
                    _ => {
                        i = nums.len();
                        None
                    }
                };
                if let Some(c) = color {
                    if n == 38 {
                        st.fg = Some(c)
                    } else {
                        st.bg = Some(c)
                    }
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pal() -> Palette {
        Palette::new(&Default::default(), &Default::default())
    }
    fn text(l: &[Line]) -> Vec<String> {
        l.iter()
            .map(|l| l.iter().map(|s| s.text.as_str()).collect())
            .collect()
    }

    #[test]
    fn colors_come_from_the_theme_and_state_carries_over_lines() {
        let p = pal();
        let base = Style::fg(p.accent);
        let l = lines("a \x1b[1;31mred\nbold\x1b[0m z", 40, base, &p);
        assert_eq!(text(&l), ["a red", "bold z"]);
        assert_eq!(
            l[0][1].style,
            Style {
                fg: p.err,
                bold: true,
                ..base
            }
        );
        assert_eq!(
            l[1][0].style,
            Style {
                fg: p.err,
                bold: true,
                ..base
            }
        );
        assert_eq!(l[1][1].style, base);
        let rgb = lines("\x1b[38;2;1;2;3mx\x1b[38;5;9my\x1b[4mz", 40, base, &p);
        assert_eq!(rgb[0][0].style.fg, Some(Color::Rgb { r: 1, g: 2, b: 3 }));
        assert_eq!(rgb[0][1].style.fg, Some(Color::AnsiValue(9)));
        assert!(rgb[0][2].style.underline);
    }

    #[test]
    fn other_escapes_are_dropped_and_do_not_count_in_the_width() {
        let p = pal();
        let junk = "\x1b[2J\x1b[10;5Hab\x1b]0;title\x07cd\x1b(Bef\x1b[31m";
        assert_eq!(text(&lines(junk, 40, Style::default(), &p)), ["abcdef"]);
        let wrapped = lines("\x1b[32maaaa bbbb cccc\x1b[0m", 9, Style::default(), &p);
        assert_eq!(text(&wrapped), ["aaaa bbbb", "cccc"]);
    }

    #[test]
    fn urls_and_osc8_links_are_clickable() {
        let p = pal();
        let l = lines(
            "go https://a.dev/x. \x1b]8;;https://b.dev\x1b\\here\x1b]8;;\x1b\\ now \x1b]8;;file:///etc\x07no\x1b]8;;\x07",
            80,
            Style::default(),
            &p,
        );
        let id = |i: usize| l[0][i].style.link.and_then(links::url);
        assert_eq!(id(1).as_deref(), Some("https://a.dev/x"));
        assert_eq!(id(3).as_deref(), Some("https://b.dev"));
        assert!(
            l[0].iter()
                .filter(|s| s.text == "no")
                .all(|s| s.style.link.is_none())
        );
    }
}
