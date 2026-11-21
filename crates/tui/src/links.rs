//! Clickable links: URLs are interned so a `Style` can carry one as a small `Copy` id.
use crate::style::{Palette, Style};
use crate::text::{Line, Span, span, width};
use std::collections::HashMap;
use std::process::{Command, Stdio};
use std::sync::{Mutex, OnceLock};

#[derive(Default)]
struct Table {
    ids: HashMap<String, u32>,
    urls: Vec<String>,
}

fn table() -> &'static Mutex<Table> {
    static T: OnceLock<Mutex<Table>> = OnceLock::new();
    T.get_or_init(Default::default)
}

pub fn intern(url: &str) -> u32 {
    let mut t = table().lock().unwrap();
    if let Some(id) = t.ids.get(url) {
        return *id;
    }
    let id = t.urls.len() as u32;
    t.urls.push(url.to_string());
    t.ids.insert(url.to_string(), id);
    id
}

pub fn url(id: u32) -> Option<String> {
    table().lock().unwrap().urls.get(id as usize).cloned()
}

/// Only these schemes are made clickable; text from the model never reaches `open` otherwise.
pub fn openable(url: &str) -> bool {
    let u = url.to_ascii_lowercase();
    url.len() < 2000
        && ["http://", "https://", "mailto:"]
            .iter()
            .any(|s| u.starts_with(s))
}

pub fn open(url: &str) {
    if !openable(url) {
        return;
    }
    let tool = if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    let child = Command::new(tool)
        .arg(url)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
    if let Ok(mut c) = child {
        std::thread::spawn(move || {
            let _ = c.wait();
        });
    }
}

/// Splits `text` into pieces; a piece with an id is a bare `http(s)://` URL.
pub fn split_urls(text: &str) -> Vec<(String, Option<u32>)> {
    let mut out = vec![];
    let mut rest = text;
    let mut prev = None::<char>;
    while let Some(at) = ["https://", "http://"]
        .iter()
        .filter_map(|s| rest.find(s))
        .min()
    {
        let before = rest[..at].chars().last().or(prev);
        let scheme = if rest[at..].starts_with("https://") {
            8
        } else {
            7
        };
        let mut end = rest[at..]
            .find(char::is_whitespace)
            .map_or(rest.len(), |i| at + i);
        loop {
            let t = &rest[at..end];
            let unbalanced = t.ends_with(')') && t.matches(')').count() > t.matches('(').count();
            match t.chars().last() {
                Some('.' | ',' | ';' | ':' | '!' | '?' | '\'' | '"') => {}
                Some(')') if unbalanced => {}
                _ => break,
            }
            end -= 1;
        }
        if before.is_some_and(char::is_alphanumeric) || end <= at + scheme {
            let keep = at + scheme;
            out.push((rest[..keep].to_string(), None));
            prev = rest[..keep].chars().last();
            rest = &rest[keep..];
            continue;
        }
        if at > 0 {
            out.push((rest[..at].to_string(), None));
        }
        out.push((rest[at..end].to_string(), Some(intern(&rest[at..end]))));
        prev = rest[..end].chars().last();
        rest = &rest[end..];
    }
    if !rest.is_empty() || out.is_empty() {
        out.push((rest.to_string(), None));
    }
    let mut merged: Vec<(String, Option<u32>)> = vec![];
    for (t, id) in out {
        match merged.last_mut() {
            Some((prev, None)) if id.is_none() => prev.push_str(&t),
            _ => merged.push((t, id)),
        }
    }
    merged
}

/// `st` for a link to `url`; one that may not be opened is only colored.
pub fn styled(url: &str, st: Style, p: &Palette) -> Style {
    Style {
        fg: p.info,
        underline: true,
        link: openable(url).then(|| intern(url)),
        ..st
    }
}

/// `text` as spans of `st`, with bare URLs underlined, colored `info` and clickable.
pub fn spans(text: &str, st: Style, p: &Palette) -> Vec<Span> {
    split_urls(text)
        .into_iter()
        .map(|(piece, id)| match id {
            Some(id) => span(
                piece,
                Style {
                    fg: p.info,
                    underline: true,
                    link: Some(id),
                    ..st
                },
            ),
            None => span(piece, st),
        })
        .collect()
}

/// Where a link is on the screen (`end` exclusive), in screen columns.
#[derive(Debug, Clone, PartialEq)]
pub struct LinkBox {
    pub row: usize,
    pub start: usize,
    pub end: usize,
    pub id: u32,
}

pub fn scan(screen: &[Line], pad: usize) -> Vec<LinkBox> {
    let mut out = vec![];
    for (row, line) in screen.iter().enumerate() {
        let mut col = pad;
        for s in line {
            let w = width(&s.text);
            if let Some(id) = s.style.link {
                out.push(LinkBox {
                    row,
                    start: col,
                    end: col + w,
                    id,
                });
            }
            col += w;
        }
    }
    out
}

/// Marks every span of the hovered link.
pub fn highlight(screen: &mut [Line], hover: Option<u32>, color: Option<crossterm::style::Color>) {
    let Some(id) = hover else { return };
    for s in screen.iter_mut().flatten() {
        if s.style.link == Some(id) {
            s.style.fg = color.or(s.style.fg);
            s.style.bold = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pieces(t: &str) -> Vec<(String, bool)> {
        split_urls(t)
            .into_iter()
            .map(|(s, i)| (s, i.is_some()))
            .collect()
    }

    #[test]
    fn finds_bare_urls_and_drops_trailing_punctuation() {
        assert_eq!(
            pieces("see https://a.dev/x?q=1, and (http://b.io/p_(1))."),
            [
                ("see ".into(), false),
                ("https://a.dev/x?q=1".into(), true),
                (", and (".into(), false),
                ("http://b.io/p_(1)".into(), true),
                (").".into(), false),
            ]
        );
        assert_eq!(pieces("no link here"), [("no link here".into(), false)]);
        assert_eq!(
            pieces("xhttps://a.dev https://"),
            [("xhttps://a.dev https://".into(), false)]
        );
    }

    #[test]
    fn interning_is_stable_and_only_web_schemes_open() {
        let a = intern("https://same.example");
        assert_eq!(a, intern("https://same.example"));
        assert_eq!(url(a).as_deref(), Some("https://same.example"));
        assert!(openable("HTTPS://x.dev") && openable("mailto:a@b.c"));
        assert!(!openable("file:///etc/passwd") && !openable("-a Calculator"));
    }
}
