use crate::kit::style::{LIST_SELECTED_PREFIX, MORE, Palette, Style};
use crate::kit::text::{Line, clip, line_width, span, width};
use crate::ui::picker::Picker;

pub(crate) fn options(pk: &mut Picker, rows: usize, w: usize, p: &Palette) -> Vec<Line> {
    pk.follow(rows);
    let inner = w.max(1);
    let mark = width(LIST_SELECTED_PREFIX);
    let half = (inner / 2).max(1);
    let mut lines: Vec<Line> = vec![];
    if pk.shown.is_empty() {
        lines.push(vec![span("no matches", Style::fg(p.muted))]);
    }
    for (n, &i) in pk.shown.iter().enumerate().skip(pk.scroll).take(rows) {
        let it = &pk.items[i];
        let sel = n == pk.selected;
        let (st, det_st) = if sel {
            (Style::fg(p.accent), Style::fg(p.muted))
        } else {
            (Style::fg(p.muted), Style::fg(p.muted))
        };
        let prefix = if sel {
            LIST_SELECTED_PREFIX.to_string()
        } else {
            " ".repeat(mark)
        };
        let mut line = vec![span(prefix, st)];
        let cells = it.cells();
        let last = cells.len() - 1;
        for (c, cell) in cells.iter().enumerate() {
            let room = inner.saturating_sub(line_width(&line));
            let later = pk.widths[c + 1..].iter().any(|&x| x > 0);
            if c == last {
                if !cell.is_empty() && room > 4 {
                    line.push(span(clip(cell, room, MORE), det_st));
                }
            } else {
                let col = if later {
                    pk.widths[c].min(half).min(room)
                } else {
                    room
                };
                let text = clip(cell, col, MORE);
                let pad = if later {
                    col.saturating_sub(width(&text)) + 2
                } else {
                    0
                };
                line.push(span(format!("{text}{}", " ".repeat(pad)), st));
            }
        }
        lines.push(line);
    }
    lines
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::picker::{Kind, item};
    use tuse_config::Ui;

    fn rows(pk: &mut Picker, w: usize) -> Vec<String> {
        let p = Palette::new(&Ui::default());
        options(pk, 20, w, &p)
            .into_iter()
            .map(|l| l.into_iter().map(|s| s.text).collect())
            .collect()
    }

    fn at(row: &str, needle: &str) -> usize {
        row[..row.find(needle).unwrap()].chars().count()
    }

    #[test]
    fn history_and_command_columns_start_at_one_position() {
        let items = (0..12)
            .map(|i| {
                let role = if i % 2 == 0 { "system" } else { "hook" };
                item(role, "text", i.to_string()).lead(i.to_string())
            })
            .collect();
        let mut pk = Picker::new(Kind::History, items);
        let out = rows(&mut pk, 60);
        assert_eq!(at(&out[0], "system"), at(&out[1], "hook"));
        assert_eq!(at(&out[0], "text"), at(&out[11], "text"));
        assert!(out[1].contains("1   hook"), "{:?}", out[1]);

        let cmds = [("new", "start"), ("resume", "open")]
            .map(|(n, d)| item(format!("/{n}"), d, n))
            .to_vec();
        let mut pk = Picker::new(Kind::Command, cmds);
        let out = rows(&mut pk, 60);
        assert_eq!(at(&out[0], "start"), at(&out[1], "open"));
    }

    #[test]
    fn a_wide_first_column_is_clipped_and_the_next_still_lines_up() {
        let items = vec![item("a".repeat(40), "one", "a"), item("b", "two", "b")];
        let mut pk = Picker::new(Kind::Resume, items);
        let out = rows(&mut pk, 40);
        assert_eq!(at(&out[0], "one"), at(&out[1], "two"));
        assert!(out[0].contains('…'));
    }
}
