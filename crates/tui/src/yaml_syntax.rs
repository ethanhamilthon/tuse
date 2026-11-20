use crate::style::{Palette, Style};
use crate::text::{Line, span};

pub fn line(s: &str, p: &Palette) -> Line {
    let mut out = vec![];
    let mut rest = s.trim_start();
    out.push(span(&s[..s.len() - rest.len()], Style::fg(p.fg)));
    if let Some(t) = rest.strip_prefix("- ") {
        out.push(span("- ", Style::fg(p.dim)));
        rest = t;
    }
    if let Some((key, value)) = rest.split_once(':')
        && !key.starts_with('#')
        && (value.is_empty() || value.starts_with(' '))
    {
        out.push(span(key, Style::fg(p.info)));
        out.push(span(":", Style::fg(p.dim)));
        rest = value;
    }
    let v = rest.trim();
    let color = if v.starts_with('#') {
        p.muted
    } else if matches!(v, "true" | "false" | "null") || v.parse::<f64>().is_ok() {
        p.accent
    } else {
        p.ok
    };
    out.push(span(rest, Style::fg(color)));
    out
}

#[test]
fn list_mapping_preserves_text_and_styles() {
    let p = Palette::new(&Default::default(), &Default::default());
    let l = line("  - count: 42", &p);
    assert_eq!(
        l.iter().map(|s| s.text.as_str()).collect::<String>(),
        "  - count: 42"
    );
    assert_eq!(l[2].style.fg, p.info);
    assert_eq!(l[4].style.fg, p.accent);
}
