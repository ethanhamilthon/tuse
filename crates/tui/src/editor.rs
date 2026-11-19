use crate::text::char_width;

/// A multi-line input buffer with a cursor.
#[derive(Default)]
pub struct Editor {
    pub buf: Vec<char>,
    pub cursor: usize,
}

impl Editor {
    pub fn text(&self) -> String {
        self.buf.iter().collect()
    }

    pub fn is_empty(&self) -> bool {
        self.buf.is_empty()
    }

    pub fn clear(&mut self) {
        self.buf.clear();
        self.cursor = 0;
    }

    pub fn insert(&mut self, s: &str) {
        for c in s.chars().filter(|c| *c != '\r') {
            self.buf.insert(self.cursor, c);
            self.cursor += 1;
        }
    }

    pub fn backspace(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
            self.buf.remove(self.cursor);
        }
    }

    pub fn delete(&mut self) {
        if self.cursor < self.buf.len() {
            self.buf.remove(self.cursor);
        }
    }

    pub fn left(&mut self) {
        self.cursor = self.cursor.saturating_sub(1);
    }

    pub fn right(&mut self) {
        self.cursor = (self.cursor + 1).min(self.buf.len());
    }

    fn line_start(&self) -> usize {
        self.buf[..self.cursor]
            .iter()
            .rposition(|c| *c == '\n')
            .map_or(0, |i| i + 1)
    }

    fn line_end(&self) -> usize {
        self.buf[self.cursor..]
            .iter()
            .position(|c| *c == '\n')
            .map_or(self.buf.len(), |i| self.cursor + i)
    }

    pub fn home(&mut self) {
        self.cursor = self.line_start();
    }

    pub fn end(&mut self) {
        self.cursor = self.line_end();
    }

    pub fn word_left(&mut self) {
        while self.cursor > 0 && self.buf[self.cursor - 1].is_whitespace() {
            self.cursor -= 1;
        }
        while self.cursor > 0 && !self.buf[self.cursor - 1].is_whitespace() {
            self.cursor -= 1;
        }
    }

    pub fn word_right(&mut self) {
        let n = self.buf.len();
        while self.cursor < n && self.buf[self.cursor].is_whitespace() {
            self.cursor += 1;
        }
        while self.cursor < n && !self.buf[self.cursor].is_whitespace() {
            self.cursor += 1;
        }
    }

    pub fn delete_word(&mut self) {
        let end = self.cursor;
        self.word_left();
        self.buf.drain(self.cursor..end);
    }

    pub fn kill_to_start(&mut self) {
        let start = self.line_start();
        self.buf.drain(start..self.cursor);
        self.cursor = start;
    }

    pub fn kill_to_end(&mut self) {
        let end = self.line_end();
        self.buf.drain(self.cursor..end);
    }

    pub fn multiline(&self) -> bool {
        self.buf.contains(&'\n')
    }

    /// Moves to the previous (`up`) or next logical line; false at the edge.
    pub fn vertical(&mut self, up: bool) -> bool {
        let col = self.cursor - self.line_start();
        if up {
            let start = self.line_start();
            if start == 0 {
                return false;
            }
            self.cursor = start - 1;
            let prev = self.line_start();
            self.cursor = (prev + col).min(start - 1);
        } else {
            let end = self.line_end();
            if end == self.buf.len() {
                return false;
            }
            self.cursor = end + 1;
            let next_end = self.line_end();
            self.cursor = (end + 1 + col).min(next_end);
        }
        true
    }

    /// The token under the cursor (from the last whitespace to the cursor) and its start.
    pub fn token(&self) -> (usize, String) {
        let start = self.buf[..self.cursor]
            .iter()
            .rposition(|c| c.is_whitespace())
            .map_or(0, |i| i + 1);
        (start, self.buf[start..self.cursor].iter().collect())
    }

    pub fn replace(&mut self, start: usize, end: usize, with: &str) {
        self.buf.splice(start..end, with.chars());
        self.cursor = start + with.chars().count();
    }

    /// Wraps the buffer to `width` columns: rows and the cursor's (row, col).
    pub fn layout(&self, width: usize) -> (Vec<String>, (usize, usize)) {
        let width = width.max(1);
        let mut rows = vec![String::new()];
        let mut w = 0;
        let mut cur = (0, 0);
        for (i, c) in self.buf.iter().enumerate() {
            if i == self.cursor {
                cur = (rows.len() - 1, w);
            }
            if *c == '\n' {
                rows.push(String::new());
                w = 0;
                continue;
            }
            let cw = char_width(*c);
            if w + cw > width {
                rows.push(String::new());
                w = 0;
            }
            rows.last_mut().unwrap().push(*c);
            w += cw;
        }
        if self.cursor == self.buf.len() {
            if w >= width {
                rows.push(String::new());
                w = 0;
            }
            cur = (rows.len() - 1, w);
        }
        (rows, cur)
    }
}
