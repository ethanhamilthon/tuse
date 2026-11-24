use crate::kit::style::{LIST_SELECTED_PREFIX, MORE, Palette, Style};
use crate::kit::text::{Line, clip, line_width, span, width};
use crate::ui::picker::Picker;

pub(crate) fn options(pk: &mut Picker, rows: usize, w: usize, p: &Palette) -> Vec<Line> {
    pk.follow(rows);
    let inner = w.max(1);
    let mark = width(LIST_SELECTED_PREFIX);
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
            (Style::fg(p.muted), Style::fg(p.muted))
        };
        let prefix = if sel {
            LIST_SELECTED_PREFIX.to_string()
        } else {
            " ".repeat(mark)
        };
        let mut line = vec![span(prefix, st)];
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
