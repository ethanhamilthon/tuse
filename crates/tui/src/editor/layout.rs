use super::Editor;
use crate::text::{char_width, width};

/// One visual row: the buffer range `start..end` it covers and the text drawn.
/// `eol` marks the last row of a logical line; only there the cursor may sit at `end`
/// (elsewhere `end` is the first char of the next row).
pub struct Row {
    pub text: String,
    pub start: usize,
    pub end: usize,
    pub eol: bool,
}

impl Editor {
    /// Wraps the buffer to `cols` columns: rows and the cursor's (row, col).
    pub fn layout(&self, cols: usize) -> (Vec<String>, (usize, usize)) {
        let (rows, cur) = self.placed(cols);
        (rows.into_iter().map(|r| r.text).collect(), cur)
    }

    /// Moves the cursor one visual row up or down, keeping the column; false at the edge.
    pub fn vertical(&mut self, up: bool, cols: usize) -> bool {
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
        self.cursor = self.at_col(&rows[to], col, cols);
        true
    }

    /// Moves the cursor to the clicked cell: `row` and `col` count from the first row and
    /// column of the text. A click below the text or past a row's end goes to the end.
    pub fn click(&mut self, row: usize, col: usize, cols: usize) {
        let cols = cols.max(1);
        let (rows, _) = self.placed(cols);
        if let Some(r) = rows.get(row).or(rows.last()) {
            let col = if row < rows.len() { col } else { usize::MAX };
            self.cursor = self.at_col(r, col, cols);
        }
    }

    /// The buffer index in `row` at display column `col`; never inside a wide char or after the row.
    fn at_col(&self, row: &Row, col: usize, cols: usize) -> usize {
        let last = if row.eol {
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

    /// Wraps `a..end` (one logical line) at word boundaries; a word longer than a row is cut.
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

    /// The rows and the cursor's (row, col). A cursor at the end of a full row
    /// gets an empty row of its own.
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
#[path = "layout_tests.rs"]
mod tests;
