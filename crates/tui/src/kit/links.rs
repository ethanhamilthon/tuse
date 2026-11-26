use crate::kit::style::{Palette, Style};
use crate::kit::text::{Line, Span, span, width};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum LinkTarget {
    Url(String),
    Path(PathBuf),
}

#[derive(Default)]
struct Table {
    ids: HashMap<LinkTarget, u32>,
    targets: Vec<LinkTarget>,
}

fn table() -> &'static Mutex<Table> {
    static T: OnceLock<Mutex<Table>> = OnceLock::new();
    T.get_or_init(Default::default)
}

pub fn intern(target: LinkTarget) -> u32 {
    let mut t = table().lock().unwrap();
    if let Some(id) = t.ids.get(&target) {
        return *id;
    }
    let id = t.targets.len() as u32;
    t.targets.push(target.clone());
    t.ids.insert(target, id);
    id
}

pub fn intern_url(url: &str) -> u32 {
    intern(LinkTarget::Url(url.to_string()))
}

pub fn intern_path(path: PathBuf) -> u32 {
    intern(LinkTarget::Path(path))
}

pub fn target(id: u32) -> Option<LinkTarget> {
    table().lock().unwrap().targets.get(id as usize).cloned()
}

pub fn openable_url(url: &str) -> bool {
    tuse_core::openable_url(url)
}

pub fn resolve_markdown_target(dest: &str, cwd: &Path) -> Option<LinkTarget> {
    let dest = dest.trim();
    if dest.is_empty() || dest.starts_with('#') {
        return None;
    }
    if openable_url(dest) {
        return Some(LinkTarget::Url(dest.to_string()));
    }
    if let Some(rest) = dest.strip_prefix("file://") {
        let stripped = rest.strip_prefix("localhost").unwrap_or(rest);
        let decoded = percent_decode(strip_anchor(stripped));
        if decoded.is_empty() {
            return None;
        }
        return Some(LinkTarget::Path(PathBuf::from(decoded)));
    }
    if has_other_scheme(dest) {
        return None;
    }
    let raw = percent_decode(strip_anchor(dest));
    if raw.is_empty() {
        return None;
    }
    let resolved = resolve_path(&raw, cwd)?;
    Some(LinkTarget::Path(resolved))
}

pub fn styled(dest: &str, st: Style, p: &Palette) -> Style {
    let link = resolve_markdown_target(dest, &p.cwd).map(intern);
    Style {
        fg: if link.is_some() { p.link } else { p.muted },
        underline: true,
        link,
        ..st
    }
}

pub fn path_span(label: impl Into<String>, path: &str, st: Style, p: &Palette) -> Span {
    let resolved = resolve_path(path, &p.cwd).filter(|rp| rp.exists());
    match resolved {
        Some(rp) => span(
            label,
            Style {
                fg: p.link,
                underline: true,
                link: Some(intern_path(rp.canonicalize().unwrap_or(rp))),
                ..st
            },
        ),
        None => span(label, st),
    }
}

pub fn spans(text: &str, st: Style, p: &Palette) -> Vec<Span> {
    split(text, &p.cwd)
        .into_iter()
        .map(|(piece, id)| match id {
            Some(id) => span(
                piece,
                Style {
                    fg: p.link,
                    underline: true,
                    link: Some(id),
                    ..st
                },
            ),
            None => span(piece, st),
        })
        .collect()
}

pub fn linkify_line(line: Line, p: &Palette) -> Line {
    let mut out = Vec::with_capacity(line.len());
    for s in line {
        if s.style.link.is_some() {
            out.push(s);
        } else {
            out.extend(spans(&s.text, s.style, p));
        }
    }
    out
}

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

pub fn highlight(screen: &mut [Line], hover: Option<u32>, color: Option<crossterm::style::Color>) {
    let Some(id) = hover else { return };
    for s in screen.iter_mut().flatten() {
        if s.style.link == Some(id) {
            s.style.fg = color;
            s.style.bold = true;
            s.style.underline = true;
        }
    }
}

pub(crate) fn resolve_path(raw: &str, cwd: &Path) -> Option<PathBuf> {
    if let Some(rest) = raw.strip_prefix("~/") {
        let home = std::env::home_dir().unwrap_or_default();
        return (!home.as_os_str().is_empty()).then(|| home.join(rest));
    }
    let p = Path::new(raw);
    if p.is_absolute() {
        Some(p.to_path_buf())
    } else if !cwd.as_os_str().is_empty() {
        Some(cwd.join(p))
    } else {
        None
    }
}

fn strip_anchor(s: &str) -> &str {
    s.split(['#', '?']).next().unwrap_or(s)
}

fn has_other_scheme(s: &str) -> bool {
    let Some((scheme, _)) = s.split_once(':') else {
        return false;
    };
    let mut chars = scheme.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    scheme.len() > 1
        && first.is_ascii_alphabetic()
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let Ok(hex) = std::str::from_utf8(&bytes[i + 1..i + 3])
            && let Ok(b) = u8::from_str_radix(hex, 16)
        {
            out.push(b);
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

pub fn split(text: &str, cwd: &Path) -> Vec<(String, Option<u32>)> {
    let mut raw = vec![];
    for (piece, url_id) in split_urls(text) {
        if url_id.is_some() {
            raw.push((piece, url_id));
        } else {
            scan_paths(&piece, cwd, &mut raw);
        }
    }
    merge_plain(raw)
}

fn split_urls(text: &str) -> Vec<(String, Option<u32>)> {
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
                Some('.' | ',' | ';' | ':' | '!' | '?' | '\'' | '"' | '>' | ']' | '}') => {}
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
        out.push((rest[at..end].to_string(), Some(intern_url(&rest[at..end]))));
        prev = rest[..end].chars().last();
        rest = &rest[end..];
    }
    if !rest.is_empty() || out.is_empty() {
        out.push((rest.to_string(), None));
    }
    out
}

fn is_delim(c: char) -> bool {
    c.is_whitespace()
        || matches!(
            c,
            '"' | '\'' | '`' | '<' | '>' | '(' | ')' | '[' | ']' | '{' | '}' | ',' | ';' | '='
        )
}

fn scan_paths(text: &str, cwd: &Path, out: &mut Vec<(String, Option<u32>)>) {
    let mut pos = 0;
    while pos < text.len() {
        let rest = &text[pos..];
        let delim_len: usize = rest
            .chars()
            .take_while(|&c| is_delim(c))
            .map(char::len_utf8)
            .sum();
        if delim_len > 0 {
            out.push((text[pos..pos + delim_len].to_string(), None));
            pos += delim_len;
            continue;
        }
        let tok_len: usize = rest
            .chars()
            .take_while(|&c| !is_delim(c))
            .map(char::len_utf8)
            .sum();
        let tok = &text[pos..pos + tok_len];
        let trimmed = trim_trailing(tok);
        let tail = &tok[trimmed.len()..];
        if let Some(resolved) = existing_path(trimmed, cwd) {
            out.push((trimmed.to_string(), Some(intern_path(resolved))));
            if !tail.is_empty() {
                out.push((tail.to_string(), None));
            }
        } else {
            out.push((tok.to_string(), None));
        }
        pos += tok_len;
    }
}

fn trim_trailing(mut s: &str) -> &str {
    while let Some('.' | ',' | ';' | ':' | '!' | '?' | '\'' | '"') = s.chars().last() {
        s = &s[..s.len() - 1];
    }
    s
}

fn existing_path(token: &str, cwd: &Path) -> Option<PathBuf> {
    if token.is_empty() || matches!(token, "/" | "." | ".." | "./" | "../" | "~/") {
        return None;
    }
    for candidate in [token, strip_line_col(token)] {
        if candidate.is_empty() || !looks_like_path(candidate) {
            continue;
        }
        if let Some(p) = resolve_path(candidate, cwd).and_then(probe) {
            return Some(p);
        }
    }
    None
}

fn probe(path: PathBuf) -> Option<PathBuf> {
    const TTL: Duration = Duration::from_secs(1);
    type Probes = HashMap<PathBuf, (Instant, Option<PathBuf>)>;
    static CACHE: OnceLock<Mutex<Probes>> = OnceLock::new();
    let cache = CACHE.get_or_init(Default::default);
    if let Some((at, hit)) = cache.lock().unwrap().get(&path)
        && at.elapsed() < TTL
    {
        return hit.clone();
    }
    let hit = path
        .exists()
        .then(|| path.canonicalize().unwrap_or_else(|_| path.clone()));
    let mut map = cache.lock().unwrap();
    if map.len() >= 4096 {
        map.retain(|_, (at, _)| at.elapsed() < TTL);
    }
    map.insert(path, (Instant::now(), hit.clone()));
    hit
}

fn looks_like_path(s: &str) -> bool {
    if s.starts_with('/') || s.starts_with("~/") || s.starts_with("./") || s.starts_with("../") {
        return s.len() > 2 || (s.starts_with('/') && s.len() > 1);
    }
    if s.contains('/') {
        return s.chars().any(char::is_alphanumeric);
    }
    if let Some((stem, ext)) = s.rsplit_once('.') {
        return !stem.is_empty()
            && stem.chars().any(char::is_alphanumeric)
            && (1..=12).contains(&ext.len())
            && ext.chars().all(|c| c.is_ascii_alphanumeric());
    }
    false
}

fn strip_line_col(s: &str) -> &str {
    let mut cur = s;
    for _ in 0..2 {
        let Some((head, tail)) = cur.rsplit_once(':') else {
            break;
        };
        if !head.is_empty() && !tail.is_empty() && tail.chars().all(|c| c.is_ascii_digit()) {
            cur = head;
        } else {
            break;
        }
    }
    cur
}

fn merge_plain(items: Vec<(String, Option<u32>)>) -> Vec<(String, Option<u32>)> {
    let mut out: Vec<(String, Option<u32>)> = vec![];
    for (t, id) in items {
        if t.is_empty() {
            continue;
        }
        match out.last_mut() {
            Some((prev, None)) if id.is_none() => prev.push_str(&t),
            _ => out.push((t, id)),
        }
    }
    if out.is_empty() {
        out.push((String::new(), None));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kit::style::Palette;
    use crate::ui::panel::Panel;

    fn pieces(t: &str, cwd: &Path) -> Vec<(String, Option<LinkTarget>)> {
        split(t, cwd)
            .into_iter()
            .map(|(s, i)| (s, i.and_then(target)))
            .collect()
    }

    #[test]
    fn finds_bare_urls_and_drops_trailing_punctuation() {
        let p = pieces(
            "see https://a.dev/x?q=1, and (http://b.io/p_(1)).",
            Path::new(""),
        );
        assert_eq!(
            p,
            [
                ("see ".into(), None),
                (
                    "https://a.dev/x?q=1".into(),
                    Some(LinkTarget::Url("https://a.dev/x?q=1".into()))
                ),
                (", and (".into(), None),
                (
                    "http://b.io/p_(1)".into(),
                    Some(LinkTarget::Url("http://b.io/p_(1)".into()))
                ),
                (").".into(), None),
            ]
        );
        assert_eq!(
            pieces("no link here", Path::new("")),
            [("no link here".into(), None)]
        );
        assert_eq!(
            pieces("xhttps://a.dev https://", Path::new("")),
            [("xhttps://a.dev https://".into(), None)]
        );
    }

    #[test]
    fn detects_relative_absolute_and_line_suffixed_paths() {
        let dir = std::env::temp_dir().join(format!("tuse-links-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        let file = dir.join("sub/note.md");
        std::fs::write(&file, "hi").unwrap();
        let canon = file.canonicalize().unwrap();
        let text = format!(
            "open sub/note.md:42:3 and ./sub/note.md and ({}) not/real.md",
            file.display()
        );
        let got = pieces(&text, &dir);
        let paths: Vec<_> = got
            .iter()
            .filter_map(|(s, t)| match t {
                Some(LinkTarget::Path(p)) => Some((s.as_str(), p.clone())),
                _ => None,
            })
            .collect();
        assert_eq!(
            paths,
            [
                ("sub/note.md:42:3", canon.clone()),
                ("./sub/note.md", canon.clone()),
                (file.to_str().unwrap(), canon.clone()),
            ]
        );
        assert_eq!(
            resolve_markdown_target("./sub/note.md#sec", &dir),
            Some(LinkTarget::Path(dir.join("./sub/note.md")))
        );
        assert!(openable_url("HTTPS://x.dev") && openable_url("mailto:a@b.c"));
        assert!(!openable_url("file:///etc/passwd") && !openable_url("-a Calculator"));
        assert_eq!(
            intern(LinkTarget::Url("https://same.example".into())),
            intern_url("https://same.example")
        );
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn wrapped_links_and_views_share_link_id_and_hit_testing() {
        let dir = std::env::temp_dir().join(format!("tuse-links-wrap-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("deep/folder/name")).unwrap();
        let file = dir.join("deep/folder/name/example.md");
        std::fs::write(&file, "hello").unwrap();
        let canon = file.canonicalize().unwrap();
        let mut p = Palette::new(&Default::default());
        p.cwd = dir.clone();

        let mut wrapped = crate::kit::rich::markdown::render(
            "Check deep/folder/name/example.md and https://example.com/very/long/url/path",
            20,
            &p,
        );
        let boxes = scan(&wrapped, 0);
        let path_boxes: Vec<_> = boxes
            .iter()
            .filter(|b| target(b.id) == Some(LinkTarget::Path(canon.clone())))
            .collect();
        assert!(
            path_boxes.len() >= 2,
            "expected wrapped path boxes: {boxes:?}"
        );
        let url_id = boxes
            .iter()
            .find(|b| {
                matches!(
                    target(b.id),
                    Some(LinkTarget::Url(ref u)) if u == "https://example.com/very/long/url/path"
                )
            })
            .expect("url box")
            .id;
        let url_boxes: Vec<_> = boxes.iter().filter(|b| b.id == url_id).collect();
        assert!(
            url_boxes.len() >= 2,
            "expected wrapped url boxes: {boxes:?}"
        );
        fn link_spans(lines: &[Line], id: u32) -> Vec<&Span> {
            lines
                .iter()
                .flatten()
                .filter(|s| s.style.link == Some(id))
                .collect()
        }
        assert!(
            link_spans(&wrapped, url_id)
                .iter()
                .all(|s| s.style.underline && !s.style.bold && s.style.fg == p.link)
        );
        highlight(&mut wrapped, Some(url_id), p.fg);
        assert!(
            link_spans(&wrapped, url_id)
                .iter()
                .all(|s| s.style.underline && s.style.bold && s.style.fg == p.fg)
        );
        highlight(&mut wrapped, Some(url_id), None);
        assert!(
            link_spans(&wrapped, url_id)
                .iter()
                .all(|s| s.style.underline && s.style.bold && s.style.fg.is_none())
        );

        let mut dup = crate::kit::rich::markdown::render(
            "https://dup.example [see `cfg`](https://dup.example) [sec](#anchor)",
            80,
            &p,
        );
        let dup_id = intern_url("https://dup.example");
        let dup_texts: Vec<_> = link_spans(&dup, dup_id)
            .into_iter()
            .map(|s| s.text.as_str())
            .collect();
        assert_eq!(dup_texts, ["https://dup.example", "see cfg"]);
        assert!(
            link_spans(&dup, dup_id)
                .iter()
                .all(|s| s.style.fg == p.link && s.style.underline)
        );
        let img = crate::kit::rich::markdown::render(
            "![pic](https://dup.example) ![](https://dup.example)",
            80,
            &p,
        );
        let img_texts: Vec<_> = link_spans(&img, dup_id)
            .into_iter()
            .map(|s| s.text.as_str())
            .collect();
        assert_eq!(img_texts, ["pic", "https://dup.example"]);
        let sec = dup.iter().flatten().find(|s| s.text == "sec").unwrap();
        assert!(sec.style.link.is_none() && sec.style.underline && sec.style.fg == p.muted);
        highlight(&mut dup, Some(dup_id), p.fg);
        assert!(
            link_spans(&dup, dup_id)
                .iter()
                .all(|s| s.style.fg == p.fg && s.style.bold && s.style.underline)
        );
        let mut p_none = Palette::new(&tuse_config::Ui {
            theme: tuse_config::Theme {
                accent: "none".into(),
                ..Default::default()
            },
            ..Default::default()
        });
        p_none.cwd = dir.clone();
        let code_link =
            crate::kit::rich::markdown::render("[`cfg`](https://dup.example)", 40, &p_none);
        assert!(
            link_spans(&code_link, dup_id)
                .iter()
                .all(|s| s.style.fg.is_none() && s.style.underline)
        );
        let ps = path_span("label", &file.display().to_string(), Style::fg(p.fg), &p);
        assert!(ps.style.link.is_some() && ps.style.fg == p.link && ps.style.underline);

        let path = "deep/folder/name/example.md";
        let text = format!("[doc]({path}) https://example.com");
        let md_block = crate::kit::rich::render::ui_markdown(&text, 60, &p);
        assert!(!scan(&md_block, 0).is_empty());
        let agent = crate::kit::entry::Entry::Assistant(text.clone());
        assert!(!scan(&crate::kit::rich::render::render(&agent, 60, &p), 0).is_empty());
        let user = crate::kit::entry::Entry::User {
            text: text.clone(),
            images: 0,
        };
        assert!(scan(&crate::kit::rich::render::render(&user, 60, &p), 0).is_empty());

        let lua = format!("read('{path}')");
        let tool_lines = crate::kit::rich::tool_view::tool(
            &lua,
            Some(&format!("{path}:1 https://example.com")),
            Some(0),
            &[tuse_core::FileDiff {
                path: path.into(),
                diff: format!("+ {path}"),
            }],
            60,
            &p,
        );
        assert!(scan(&tool_lines, 0).is_empty());
        let fenced_lua = crate::kit::rich::markdown::render(&format!("```lua\n{lua}\n```"), 60, &p);
        assert!(scan(&fenced_lua, 0).is_empty());

        for mut panel in [
            Panel::new(&text),
            Panel::lua(lua),
            Panel::markdown_unlinked(&text),
        ] {
            assert!(scan(&panel.lines(60, 10, &p), 0).is_empty());
        }
        for mut panel in [
            Panel::markdown(&text),
            Panel::styled(vec![vec![crate::kit::text::span(path, Default::default())]]),
        ] {
            assert!(
                scan(&panel.lines(60, 10, &p), 0)
                    .iter()
                    .any(|b| matches!(target(b.id), Some(LinkTarget::Path(path)) if path.canonicalize().ok().as_ref() == Some(&canon)))
            );
        }
        std::fs::remove_dir_all(dir).ok();
    }
}
