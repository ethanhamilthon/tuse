use std::path::PathBuf;

mod layout;
mod selection;

/// First code point of the private-use range that stands in for atoms.
const ATOM_BASE: u32 = 0xF0000;

/// A file, prompt or image in the input. The buffer holds one placeholder
/// char for it, so deleting and the arrows treat it as a single letter.
pub struct Atom {
    pub label: String,
    pub image: Option<PathBuf>,
}

/// A multi-line input buffer with a cursor.
#[derive(Default)]
pub struct Editor {
    pub buf: Vec<char>,
    pub cursor: usize,
    pub anchor: Option<usize>,
    atoms: Vec<Atom>,
}

fn atom_index(c: char) -> Option<usize> {
    (c as u32).checked_sub(ATOM_BASE).map(|i| i as usize)
}

impl Editor {
    /// The text with every atom written out as its label.
    pub fn text(&self) -> String {
        self.range_text(0, self.buf.len())
    }

    pub fn insert_image(&mut self, path: PathBuf) {
        let n = 1 + self.atoms.iter().filter(|a| a.image.is_some()).count();
        self.insert_atom(&format!("[Image #{n}]"), Some(path));
    }

    pub fn text_expanded(&self) -> String {
        self.buf
            .iter()
            .map(|c| match self.atom(*c) {
                Some(Atom {
                    label,
                    image: Some(path),
                }) => format!("{}: {}]", label.trim_end_matches(']'), path.display()),
                Some(a) => a.label.clone(),
                None => c.to_string(),
            })
            .collect()
    }

    fn range_text(&self, a: usize, b: usize) -> String {
        self.buf[a..b]
            .iter()
            .map(|c| match self.atom(*c) {
                Some(a) => a.label.clone(),
                None => c.to_string(),
            })
            .collect()
    }

    fn atom(&self, c: char) -> Option<&Atom> {
        atom_index(c).and_then(|i| self.atoms.get(i))
    }

    pub fn insert_atom(&mut self, label: &str, image: Option<PathBuf>) {
        self.delete_selection();
        let c = char::from_u32(ATOM_BASE + self.atoms.len() as u32).unwrap_or('?');
        self.atoms.push(Atom {
            label: label.to_string(),
            image,
        });
        self.buf.insert(self.cursor, c);
        self.cursor += 1;
    }

    /// Paths of the images placed in the input.
    pub fn images(&self) -> Vec<PathBuf> {
        self.buf
            .iter()
            .filter_map(|c| self.atom(*c)?.image.clone())
            .collect()
    }

    pub fn is_empty(&self) -> bool {
        self.buf.is_empty()
    }

    pub fn clear(&mut self) {
        self.clear_selection();
        self.buf.clear();
        self.atoms.clear();
        self.cursor = 0;
    }

    pub fn insert(&mut self, s: &str) {
        self.delete_selection();
        for c in s.chars().filter(|c| *c != '\r' && atom_index(*c).is_none()) {
            self.buf.insert(self.cursor, c);
            self.cursor += 1;
        }
    }

    pub fn backspace(&mut self) {
        if !self.delete_selection() && self.cursor > 0 {
            self.cursor -= 1;
            self.buf.remove(self.cursor);
        }
    }

    pub fn delete(&mut self) {
        if !self.delete_selection() && self.cursor < self.buf.len() {
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
        if self.delete_selection() {
            return;
        }
        let end = self.cursor;
        self.word_left();
        self.buf.drain(self.cursor..end);
    }

    pub fn kill_to_start(&mut self) {
        if self.delete_selection() {
            return;
        }
        let start = self.line_start();
        self.buf.drain(start..self.cursor);
        self.cursor = start;
    }

    pub fn kill_to_end(&mut self) {
        if self.delete_selection() {
            return;
        }
        let end = self.line_end();
        self.buf.drain(self.cursor..end);
    }

    /// The token under the cursor (from the last whitespace to the cursor) and its start.
    /// Atoms end a token like whitespace does.
    pub fn token(&self) -> (usize, String) {
        let start = self.buf[..self.cursor]
            .iter()
            .rposition(|c| c.is_whitespace() || atom_index(*c).is_some())
            .map_or(0, |i| i + 1);
        (start, self.buf[start..self.cursor].iter().collect())
    }

    pub fn replace(&mut self, start: usize, end: usize, with: &str) {
        self.clear_selection();
        self.buf.splice(start..end, with.chars());
        self.cursor = start + with.chars().count();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atom_is_one_letter() {
        let mut e = Editor::default();
        e.insert("a ");
        e.insert_atom("@src/main.rs", None);
        e.insert(" b");
        assert_eq!(e.text(), "a @src/main.rs b");
        assert_eq!(e.buf.len(), 5);
        e.left();
        e.left();
        e.backspace();
        assert_eq!(e.text(), "a  b");
        let mut e = Editor::default();
        e.insert_image("/x.png".into());
        assert_eq!(e.text(), "[Image #1]");
        assert_eq!(e.text_expanded(), "[Image #1: /x.png]");
        e.insert_image("/y.png".into());
        assert_eq!(e.text(), "[Image #1][Image #2]");
        e.backspace();
        e.insert_image("/z.png".into());
        assert_eq!(e.text(), "[Image #1][Image #3]");
        e.backspace();
        assert_eq!(e.images(), [PathBuf::from("/x.png")]);
        assert_eq!(e.token().1, "");
    }
}
