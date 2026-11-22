use crate::app::App;
use crate::picker::Picker;
use crate::style::{LIST_SELECTED_PREFIX, MORE, Palette, RULE_CHAR, Style, TRACE_ROLE_WIDTH};
use crate::text::{Line, clip, line_width, span, width};

/// The look every list and text view shares: a rule on top, a title line (when there
/// is a title), then `rows`, all on the list background and `width` wide.
pub fn framed(title: &str, rows: Vec<Line>, w: usize, p: &Palette) -> Vec<Line> {
    let base = Style::fg(p.fg).on(p.bg);
    let mut out: Vec<Line> = vec![];
    if !title.is_empty() {
        let text = clip(title, w, MORE);
        out.push(vec![span(text, Style::fg(p.accent).bold())]);
    }
    for l in rows {
        let line: Line = l
            .into_iter()
            .map(|mut s| {
                s.style.bg = s.style.bg.or(p.bg);
                s
            })
            .collect();
        out.push(line);
    }
    for l in &mut out {
        let fill = w.saturating_sub(line_width(l));
        l.push(span(" ".repeat(fill), base));
    }
    out.insert(0, vec![span(RULE_CHAR.repeat(w), Style::fg(p.border))]);
    out
}

/// The title of a picker: its name and the position; the query is in the input.
fn title_of(pk: &Picker) -> String {
    if pk.title.is_empty() || pk.shown.is_empty() {
        return pk.title.clone();
    }
    format!("{} {}/{}", pk.title, pk.selected + 1, pk.shown.len())
}

/// The open picker as screen lines, at most `max` of them. Stores the number of rows
/// it shows in `app.list_rows`, which the keys use to keep the selection in view.
pub fn picker(app: &mut App, w: usize, max: usize) -> Vec<Line> {
    let p = &app.palette;
    let Some(pk) = &mut app.picker else {
        return vec![];
    };
    let title = title_of(pk);
    let rows = max
        .saturating_sub(1 + usize::from(!title.is_empty()))
        .max(1);
    let lines = options(pk, rows, w, p);
    app.list_rows = rows;
    framed(&title, lines, w, p)
}

pub(crate) fn options(pk: &mut Picker, rows: usize, w: usize, p: &Palette) -> Vec<Line> {
    pk.follow(rows);
    let inner = w.max(1);
    let mark = width(LIST_SELECTED_PREFIX);
    let tag_w = pk.items.iter().map(|i| width(&i.tag)).max().unwrap_or(0);
    let tag_w = tag_w.min(TRACE_ROLE_WIDTH);
    let mut lines: Vec<Line> = vec![];
    if pk.shown.is_empty() {
        lines.push(vec![span("no matches", Style::fg(p.dim))]);
    }
    for (n, &i) in pk.shown.iter().enumerate().skip(pk.scroll).take(rows) {
        let it = &pk.items[i];
        let sel = n == pk.selected;
        let (st, det_st) = if sel {
            (Style::fg(p.accent), Style::fg(p.dim))
        } else {
            (Style::fg(p.border), Style::fg(p.border))
        };
        let prefix = if sel {
            LIST_SELECTED_PREFIX.to_string()
        } else {
            " ".repeat(mark)
        };
        let mut line = vec![span(prefix, st)];
        if tag_w > 0 {
            line.push(span(format!("{:<tag_w$} ", it.tag), st));
        }
        let room = inner.saturating_sub(line_width(&line));
        let label = clip(&it.label.replace('\n', " "), room, MORE);
        line.push(span(label, st));
        let room = inner.saturating_sub(line_width(&line) + 2);
        if !it.detail.is_empty() && room > 4 {
            line.push(span(format!("  {}", clip(&it.detail, room, MORE)), det_st));
        }
        lines.push(line);
    }
    lines
}
