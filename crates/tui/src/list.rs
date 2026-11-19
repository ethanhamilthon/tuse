use crate::app::App;
use crate::picker::Picker;
use crate::style::{Palette, Style};
use crate::text::{Line, clip, line_width, span, width};
use crossterm::style::Color;

/// The look every list and text view shares: a rule on top, a title line (when there
/// is a title), then `rows`, all on the list background and `width` wide.
pub fn framed(title: &str, rows: Vec<Line>, w: usize, p: &Palette) -> Vec<Line> {
    let t = &p.t;
    let base = Style::fg(p.list_fg).on(p.list_bg);
    let pad = " ".repeat(t.list_pad_x as usize);
    let mut out: Vec<Line> = vec![];
    if !title.is_empty() {
        let text = clip(&format!("{pad}{title}"), w, &t.more);
        out.push(vec![span(
            text,
            Style::fg(p.list_title).on(p.list_bg).bold(),
        )]);
    }
    for l in rows {
        let mut line = vec![span(pad.clone(), base)];
        line.extend(l.into_iter().map(|mut s| {
            s.style.bg = s.style.bg.or(p.list_bg);
            s
        }));
        out.push(line);
    }
    for l in &mut out {
        let fill = w.saturating_sub(line_width(l));
        l.push(span(" ".repeat(fill), base));
    }
    out.insert(0, vec![span(t.rule_char.repeat(w), Style::fg(p.rule))]);
    out
}

/// The title of a picker: its name, the position and the query (or the typed answer).
fn title_of(pk: &Picker) -> String {
    if pk.title.is_empty() {
        return String::new();
    }
    let at = if pk.is_free() || pk.shown.is_empty() {
        String::new()
    } else {
        format!(" {}/{}", pk.selected + 1, pk.shown.len())
    };
    let q = if pk.query.is_empty() {
        String::new()
    } else {
        format!(" › {}", pk.query)
    };
    format!("{}{at}{q}", pk.title)
}

/// The open picker as screen lines, at most `max` of them. Stores the number of rows
/// it shows in `app.list_rows`, which the keys use to keep the selection in view.
pub fn picker(app: &mut App, w: usize, max: usize) -> Vec<Line> {
    let p = &app.palette;
    let t = &p.t;
    let Some(pk) = &mut app.picker else {
        return vec![];
    };
    let title = title_of(pk);
    let rows = max
        .saturating_sub(1 + usize::from(!title.is_empty()))
        .max(1);
    app.list_rows = rows;
    pk.follow(rows);
    let pad = t.list_pad_x as usize;
    let inner = w.saturating_sub(pad * 2).max(1);
    let mark = width(&t.list_selected_prefix);
    let tag_w = pk.items.iter().map(|i| width(&i.tag)).max().unwrap_or(0);
    let tag_w = tag_w.min(t.trace_role_width as usize);
    let mut lines: Vec<Line> = vec![];
    if pk.shown.is_empty() {
        lines.push(vec![span("no matches", Style::fg(p.list_detail))]);
    }
    for (n, &i) in pk.shown.iter().enumerate().skip(pk.scroll).take(rows) {
        let it = &pk.items[i];
        let sel = n == pk.selected;
        let on = Style::fg(p.list_selected_fg).on(p.list_selected_bg);
        let st = if sel { on } else { Style::fg(p.list_fg) };
        let prefix = if sel {
            t.list_selected_prefix.clone()
        } else {
            " ".repeat(mark)
        };
        let mut line = vec![span(prefix, st)];
        if tag_w > 0 {
            let tag = format!("{:<tag_w$} ", it.tag);
            let color = if sel {
                on
            } else {
                Style::fg(role_color(p, &it.tag))
            };
            line.push(span(tag, color));
        }
        let room = inner.saturating_sub(line_width(&line));
        let label = clip(&it.label.replace('\n', " "), room, &t.more);
        line.push(span(label, st));
        let room = inner.saturating_sub(line_width(&line) + 2);
        if !it.detail.is_empty() && room > 4 {
            let det = if sel { on } else { Style::fg(p.list_detail) };
            line.push(span(format!("  {}", clip(&it.detail, room, &t.more)), det));
        }
        if sel {
            let fill = inner.saturating_sub(line_width(&line));
            line.push(span(" ".repeat(fill), on));
        }
        lines.push(line);
    }
    framed(&title, lines, w, p)
}

fn role_color(p: &Palette, role: &str) -> Option<Color> {
    match role {
        "user" => p.user,
        "agent" => p.assistant,
        "thinking" => p.reasoning,
        "python" | "tool" => p.tool,
        "output" => p.tool_output,
        "error" => p.error,
        "notice" => p.notice,
        _ => p.dim,
    }
}
