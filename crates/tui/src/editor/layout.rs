use super::Editor;
use crate::text::{char_width, width};

impl Editor {
    /// Wraps the buffer to `cols` columns: rows and the cursor's (row, col).
    /// An atom is drawn as its label and moves to the next row whole when it fits there.
    pub fn layout(&self, cols: usize) -> (Vec<String>, (usize, usize)) {
        let cols = cols.max(1);
        let mut rows = vec![String::new()];
        let mut w = 0;
        let mut cur = (0, 0);
        for (i, c) in self.buf.iter().enumerate() {
            if *c == '\n' {
                if i == self.cursor {
                    cur = (rows.len() - 1, w);
                }
                rows.push(String::new());
                w = 0;
                continue;
            }
            let text = match self.atom(*c) {
                Some(a) => a.label.clone(),
                None => c.to_string(),
            };
            let tw = width(&text);
            if w > 0 && tw > 1 && w + tw > cols && tw <= cols {
                rows.push(String::new());
                w = 0;
            }
            if i == self.cursor {
                cur = (rows.len() - 1, w);
            }
            for ch in text.chars() {
                let cw = char_width(ch);
                if w + cw > cols {
                    rows.push(String::new());
                    w = 0;
                }
                rows.last_mut().unwrap().push(ch);
                w += cw;
            }
        }
        if self.cursor == self.buf.len() {
            if w >= cols {
                rows.push(String::new());
                w = 0;
            }
            cur = (rows.len() - 1, w);
        }
        (rows, cur)
    }
}
