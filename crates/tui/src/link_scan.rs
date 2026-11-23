//! Shared scanner that finds bare URLs and existing local file paths in plain text.

use crate::links::{intern_path, intern_url, resolve_path};
use std::path::{Path, PathBuf};

/// Splits `text` into pieces; pieces with `Some(id)` are clickable URLs or local file paths.
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

/// Checks whether `token` (optionally with `:line` or `:line:col`) looks like a file path
/// and resolves to an existing path on disk.
fn existing_path(token: &str, cwd: &Path) -> Option<PathBuf> {
    if token.is_empty() || matches!(token, "/" | "." | ".." | "./" | "../" | "~/") {
        return None;
    }
    for candidate in [token, strip_line_col(token)] {
        if candidate.is_empty() || !looks_like_path(candidate) {
            continue;
        }
        if let Some(p) = resolve_path(candidate, cwd)
            && p.exists()
        {
            return Some(p.canonicalize().unwrap_or(p));
        }
    }
    None
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

/// Strips trailing `:line` or `:line:col` suffix when all parts after `:` are ASCII digits.
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
#[path = "links_tests.rs"]
mod tests;
