use serde_json::Value;
use std::path::Path;

pub(crate) fn error_line(text: &str) -> String {
    let slice = match text.find("\nstack traceback:") {
        Some(i) => &text[..i],
        None => text,
    };
    last_line(slice).unwrap_or("failed").to_string()
}

pub(crate) fn last_line(text: &str) -> Option<&str> {
    text.lines().rev().map(str::trim).find(|l| !l.is_empty())
}

pub(crate) fn compact(mut v: Value) -> Value {
    if let Value::Object(m) = &mut v {
        m.retain(|_, val| !val.is_null());
    }
    v
}

pub fn tokens(n: u64) -> String {
    let (v, unit) = match n {
        n if n >= 1_000_000 => (n as f64 / 1e6, "m"),
        n if n >= 1000 => (n as f64 / 1e3, "k"),
        n => return n.to_string(),
    };
    let s = format!("{v:.1}");
    format!("{}{unit}", s.strip_suffix(".0").unwrap_or(&s))
}

pub(crate) fn image_mime(p: &Path) -> Option<&'static str> {
    let ext = p.extension()?.to_str()?.to_ascii_lowercase();
    Some(match ext.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        _ => return None,
    })
}
