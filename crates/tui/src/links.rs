//! Interactive links and local file paths: interned targets, styling, hit testing and hover.
use crate::style::{Palette, Style};
use crate::text::{Line, Span, span, width};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

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

/// Resolves a Markdown link destination into a URL or local path target.
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

/// `st` for a Markdown link to `dest`; unsupported schemes are colored without a click target.
pub fn styled(dest: &str, st: Style, p: &Palette) -> Style {
    Style {
        fg: p.info,
        underline: true,
        link: resolve_markdown_target(dest, &p.cwd).map(intern),
        ..st
    }
}

/// A single span pointing directly at a local path (even if the path has spaces or is clipped).
pub fn path_span(label: impl Into<String>, path: &str, st: Style, p: &Palette) -> Span {
    let resolved = resolve_path(path, &p.cwd).filter(|rp| rp.exists());
    match resolved {
        Some(rp) => span(
            label,
            Style {
                fg: p.info,
                underline: true,
                link: Some(intern_path(rp.canonicalize().unwrap_or(rp))),
                ..st
            },
        ),
        None => span(label, st),
    }
}

/// `text` as spans of `st`, with bare URLs and existing file paths underlined, colored `info`
/// and clickable.
pub fn spans(text: &str, st: Style, p: &Palette) -> Vec<Span> {
    crate::link_scan::split(text, &p.cwd)
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

/// Applies URL/path detection to every non-link span of `line`.
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

pub(crate) fn resolve_path(raw: &str, cwd: &Path) -> Option<PathBuf> {
    if let Some(rest) = raw.strip_prefix("~/") {
        let home = tuse_config::home();
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
