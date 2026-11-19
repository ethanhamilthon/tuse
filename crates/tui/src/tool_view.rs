use crate::style::{Palette, Style};
use crate::text::{Line, span};

/// A tool call in the chat: its input, then its output; the diff of `edit`
/// and `write` is colored.
pub fn tool(
    name: &str,
    input: &str,
    output: Option<&str>,
    code: Option<i32>,
    width: usize,
    p: &Palette,
) -> Vec<Line> {
    let t = &p.t;
    let failed = output.is_some() && code != Some(0);
    let head = Style::fg(if failed { p.tool_error } else { p.tool });
    let pad = " ".repeat(crate::text::width(&t.tool_prefix));
    let shown = match name {
        "python" | "bash" => input.trim().to_string(),
        other => format!("{other} {}", input.trim()),
    };
    let mut lines = crate::render::plain(&shown, width, head, &t.tool_prefix, &pad);
    let keep = tuse_config::limit(t.tool_line, lines.len());
    if keep < lines.len() {
        lines.truncate(keep);
        if keep > 0 {
            lines.push(vec![span(format!("{pad}{}", t.more), Style::fg(p.dim))]);
        }
    }
    let Some(out) = output else { return lines };
    let out = crate::render::clean(out);
    let all: Vec<&str> = out.trim_end().lines().collect();
    let diff = name == "edit" && !failed;
    let limit = if diff { t.diff_line } else { t.output_line };
    let shown = tuse_config::limit(limit, all.len());
    for l in all.iter().take(shown) {
        let color = match (diff, l.chars().next()) {
            (true, Some('+')) => p.diff_add,
            (true, Some('-')) => p.diff_del,
            _ => p.tool_output,
        };
        let st = Style::fg(color);
        let l = crate::text::clip(l, width.saturating_sub(pad.len()), &t.more);
        lines.push(vec![span(pad.clone(), st), span(l, st)]);
    }
    if shown > 0 && shown < all.len() {
        let more = format!("{pad}{} {} more lines", t.more, all.len() - shown);
        lines.push(vec![span(more, Style::fg(p.dim))]);
    }
    lines
}
