use super::{Editor, View};
use crate::kit::text::{char_width, width};

pub struct Row {
    pub text: String,
    pub start: usize,
    pub end: usize,
    pub eol: bool,
}

impl Editor {
    pub fn view(&self, cols: usize) -> View {
        let (rows, cursor) = self.placed(cols);
        View {
            cursor,
            selected: self.selected_columns(&rows, cols),
            atoms: self.atom_columns(&rows, cols),
            rows: rows.into_iter().map(|r| r.text).collect(),
        }
    }

    fn selected_columns(&self, rows: &[Row], cols: usize) -> Vec<(usize, usize)> {
        let Some((a, b)) = self.selection_range() else {
            return vec![];
        };
        rows.iter()
            .map(|r| {
                let col = |index: usize| {
                    (r.start..index.clamp(r.start, r.end))
                        .map(|i| self.unit(i, cols).1)
                        .sum::<usize>()
                        .min(width(&r.text))
                };
                (col(a), col(b))
            })
            .collect()
    }

    fn atom_columns(&self, rows: &[Row], cols: usize) -> Vec<Vec<(usize, usize)>> {
        rows.iter()
            .map(|r| {
                let mut col = 0;
                let mut out = vec![];
                for i in r.start..r.end.min(self.buf.len()) {
                    let w = self.unit(i, cols).1;
                    if self.atom(self.buf[i]).is_some() {
                        out.push((col, col + w));
                    }
                    col += w;
                }
                out
            })
            .collect()
    }

    pub(super) fn vertical(&mut self, up: bool, cols: usize) -> bool {
        let cols = cols.max(1);
        let (rows, (r, col)) = self.placed(cols);
        let to = if up {
            match r.checked_sub(1) {
                Some(t) => t,
                None => return false,
            }
        } else if r + 1 < rows.len() {
            r + 1
        } else {
            return false;
        };
        self.cursor = self.at_col(&rows[to], col, cols, false);
        true
    }

    pub(super) fn click(&mut self, row: usize, col: usize, cols: usize, drag: bool) {
        let cols = cols.max(1);
        let (rows, _) = self.placed(cols);
        if let Some(r) = rows.get(row).or(rows.last()) {
            let col = if row < rows.len() { col } else { usize::MAX };
            self.cursor = self.at_col(r, col, cols, drag);
        }
    }

    fn at_col(&self, row: &Row, col: usize, cols: usize, drag: bool) -> usize {
        let last = if row.eol || drag {
            row.end
        } else {
            row.end.saturating_sub(1)
        };
        let (mut at, mut w) = (row.start, 0);
        while at < last {
            let uw = self.unit(at, cols).1;
            if w + uw > col {
                break;
            }
            w += uw;
            at += 1;
        }
        at
    }

    fn unit(&self, i: usize, cols: usize) -> (String, usize) {
        let c = self.buf[i];
        let Some(a) = self.atom(c) else {
            return (c.to_string(), char_width(c));
        };
        let mut text = String::new();
        let mut w = 0;
        for ch in a.label.chars() {
            if w + char_width(ch) > cols {
                break;
            }
            w += char_width(ch);
            text.push(ch);
        }
        (text, w.min(width(&a.label)))
    }

    fn is_space(&self, i: usize) -> bool {
        self.atom(self.buf[i]).is_none() && self.buf[i].is_whitespace()
    }

    fn wrap_line(&self, a: usize, end: usize, cols: usize, rows: &mut Vec<Row>) {
        let (mut start, mut w, mut text) = (a, 0, String::new());
        let mut flush = |start: &mut usize, at: usize, w: &mut usize, text: &mut String| {
            rows.push(Row {
                text: std::mem::take(text),
                start: *start,
                end: at,
                eol: false,
            });
            *start = at;
            *w = 0;
        };
        let mut k = a;
        while k < end {
            let mut m = k;
            let mut word_w = 0;
            while m < end && !self.is_space(m) {
                word_w += self.unit(m, cols).1;
                m += 1;
            }
            let mut e = m;
            while e < end && self.is_space(e) {
                e += 1;
            }
            if w > 0 && w + word_w > cols {
                flush(&mut start, k, &mut w, &mut text);
            }
            for i in k..m {
                let (t, uw) = self.unit(i, cols);
                if w > 0 && w + uw > cols {
                    flush(&mut start, i, &mut w, &mut text);
                }
                text.push_str(&t);
                w += uw;
            }
            for i in m..e {
                let (t, uw) = self.unit(i, cols);
                if w + uw <= cols {
                    text.push_str(&t);
                    w += uw;
                }
            }
            k = e;
        }
        flush(&mut start, end, &mut w, &mut text);
        rows.last_mut().unwrap().eol = true;
    }

    fn placed(&self, cols: usize) -> (Vec<Row>, (usize, usize)) {
        let cols = cols.max(1);
        let mut rows = vec![];
        let (mut i, n) = (0, self.buf.len());
        loop {
            let end = self.buf[i..]
                .iter()
                .position(|c| *c == '\n')
                .map_or(n, |p| i + p);
            self.wrap_line(i, end, cols, &mut rows);
            if end == n {
                break;
            }
            i = end + 1;
        }
        let c = self.cursor;
        let r = rows
            .iter()
            .position(|r| (r.start <= c && c < r.end) || (r.eol && c == r.end))
            .unwrap_or(rows.len() - 1);
        let raw: usize = (rows[r].start..c).map(|i| self.unit(i, cols).1).sum();
        if raw >= cols && rows[r].eol && c == rows[r].end {
            rows[r].eol = false;
            rows.insert(
                r + 1,
                Row {
                    text: String::new(),
                    start: c,
                    end: c,
                    eol: true,
                },
            );
            return (rows, (r + 1, 0));
        }
        (rows, (r, raw.min(cols - 1)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ed(s: &str) -> Editor {
        let mut e = Editor::default();
        e.insert(s);
        e
    }

    #[test]
    fn wraps_at_words_and_cuts_long_ones() {
        let e = ed("hello big world");
        assert_eq!(e.view(8).rows, ["hello ", "big ", "world"]);
        assert_eq!(ed("abcdefghij").view(4).rows, ["abcd", "efgh", "ij"]);
        assert_eq!(ed("ab cdefghij").view(4).rows, ["ab ", "cdef", "ghij", ""]);
        assert_eq!(ed("ab cdefghi").view(4).rows, ["ab ", "cdef", "ghi"]);
    }

    #[test]
    fn cursor_follows_wrapped_rows() {
        let mut e = ed("hello big world");
        assert_eq!(e.view(8).cursor, (2, 5));
        e.cursor = 6;
        assert_eq!(e.view(8).cursor, (1, 0));
        let mut e = ed("abcd");
        assert_eq!(e.view(4).cursor, (1, 0));
        e.cursor = 2;
        assert_eq!(e.view(4).cursor, (0, 2));
    }

    #[test]
    fn view_aligns_selection_and_atoms_across_wrapped_rows() {
        let mut e = ed("ab ");
        e.insert_atom("@x", super::super::AtomKind::File("x".into()));
        e.insert("語");
        e.select_all();
        let view = e.view(4);
        assert_eq!(view.rows, ["ab ", "@x語", ""]);
        assert_eq!(view.cursor, (2, 0));
        assert_eq!(view.selected, [(0, 3), (0, 4), (0, 0)]);
        assert_eq!(view.atoms, [vec![], vec![(0, 2)], vec![]]);
    }

    #[test]
    fn a_click_lands_on_the_cell_in_wrapped_wide_and_atom_text() {
        let mut e = ed("hello big world");
        e.click(1, 2, 8, false);
        assert_eq!(e.cursor, 8);
        e.click(1, 40, 8, false);
        assert_eq!(e.cursor, 9);
        e.click(9, 0, 8, false);
        assert_eq!(e.cursor, 15);
        let mut e = ed("日本語");
        e.click(0, 3, 10, false);
        assert_eq!(e.cursor, 1);
        e.click(0, 4, 10, false);
        assert_eq!(e.cursor, 2);
        let mut e = ed("a ");
        e.insert_image("/x.png".into());
        e.insert("b");
        e.click(0, 5, 40, false);
        assert_eq!(e.cursor, 2);
        e.click(0, 12, 40, false);
        assert_eq!(e.cursor, 3);
    }

    #[test]
    fn arrows_walk_visual_rows_not_newlines() {
        let mut e = ed("hello big world");
        assert!(e.vertical(true, 8));
        assert_eq!(e.cursor, 9);
        assert!(e.vertical(true, 8));
        assert_eq!(e.cursor, 3);
        assert!(!e.vertical(true, 8));
        assert!(e.vertical(false, 8));
        assert!(e.vertical(false, 8));
        assert!(!e.vertical(false, 8));
        let mut e = ed("ab\n\ncd");
        e.cursor = 1;
        assert!(e.vertical(false, 8));
        assert_eq!(e.cursor, 3);
        assert!(e.vertical(false, 8));
        assert_eq!(e.cursor, 4);
    }
}
