mod atoms;
mod layout;
mod selection;

pub use atoms::{Atom, AtomKind};

const ATOM_BASE: u32 = 0xF0000;

#[derive(Default)]
pub struct Editor {
    buf: Vec<char>,
    cursor: usize,
    anchor: Option<usize>,
    atoms: Vec<Atom>,
}

fn atom_index(c: char) -> Option<usize> {
    (c as u32).checked_sub(ATOM_BASE).map(|i| i as usize)
}

#[derive(Debug, Clone, Copy)]
pub enum Edit {
    Backspace,
    Delete,
    Left,
    Right,
    WordLeft,
    WordRight,
    Home,
    End,
    Up,
    Down,
    DeleteWord,
    KillToStart,
    KillToEnd,
    SelectAll,
    Anchor,
    AnchorAt(usize),
    Deselect,
    DeleteSelection,
    Click { row: usize, col: usize },
}

pub struct View {
    pub rows: Vec<String>,
    pub cursor: (usize, usize),
    pub selected: Vec<(usize, usize)>,
    pub atoms: Vec<Vec<(usize, usize)>>,
}

pub struct Refs {
    pub prompts: Vec<String>,
    pub files: Vec<std::path::PathBuf>,
    pub images: Vec<(usize, std::path::PathBuf)>,
}

impl Editor {
    pub fn apply(&mut self, edit: Edit, cols: usize) {
        match edit {
            Edit::Backspace => self.backspace(),
            Edit::Delete => self.delete(),
            Edit::Left => self.left(),
            Edit::Right => self.right(),
            Edit::WordLeft => self.word_left(),
            Edit::WordRight => self.word_right(),
            Edit::Home => self.home(),
            Edit::End => self.end(),
            Edit::Up => {
                self.vertical(true, cols);
            }
            Edit::Down => {
                self.vertical(false, cols);
            }
            Edit::DeleteWord => self.delete_word(),
            Edit::KillToStart => self.kill_to_start(),
            Edit::KillToEnd => self.kill_to_end(),
            Edit::SelectAll => self.select_all(),
            Edit::Anchor => {
                self.anchor.get_or_insert(self.cursor);
            }
            Edit::AnchorAt(index) => self.anchor = Some(index),
            Edit::Deselect => self.clear_selection(),
            Edit::DeleteSelection => {
                self.delete_selection();
            }
            Edit::Click { row, col } => self.click(row, col, cols),
        }
    }

    pub fn view(&self, cols: usize) -> View {
        let (rows, cursor) = self.layout(cols);
        View {
            rows,
            cursor,
            selected: self.selected_columns(cols),
            atoms: self.atom_columns(cols),
        }
    }

    pub fn refs(&self) -> Refs {
        Refs {
            prompts: self.prompts(),
            files: self.files(),
            images: self.images(),
        }
    }

    pub fn cursor(&self) -> usize {
        self.cursor
    }

    pub fn snapshot(&self) -> Vec<char> {
        self.buf.clone()
    }

    pub fn differs_from(&self, before: &[char]) -> bool {
        self.buf != before
    }

    pub fn has_selection(&self) -> bool {
        self.selection_range().is_some()
    }

    #[cfg(test)]
    pub fn set_cursor(&mut self, at: usize) {
        self.cursor = at;
    }

    pub fn text(&self) -> String {
        self.range_text(0, self.buf.len())
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

    pub(super) fn backspace(&mut self) {
        if !self.delete_selection() && self.cursor > 0 {
            self.cursor -= 1;
            self.buf.remove(self.cursor);
        }
    }

    pub(super) fn delete(&mut self) {
        if !self.delete_selection() && self.cursor < self.buf.len() {
            self.buf.remove(self.cursor);
        }
    }

    pub(super) fn left(&mut self) {
        self.cursor = self.cursor.saturating_sub(1);
    }

    pub(super) fn right(&mut self) {
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

    pub(super) fn home(&mut self) {
        self.cursor = self.line_start();
    }

    pub(super) fn end(&mut self) {
        self.cursor = self.line_end();
    }

    pub(super) fn word_left(&mut self) {
        while self.cursor > 0 && self.buf[self.cursor - 1].is_whitespace() {
            self.cursor -= 1;
        }
        while self.cursor > 0 && !self.buf[self.cursor - 1].is_whitespace() {
            self.cursor -= 1;
        }
    }

    pub(super) fn word_right(&mut self) {
        let n = self.buf.len();
        while self.cursor < n && self.buf[self.cursor].is_whitespace() {
            self.cursor += 1;
        }
        while self.cursor < n && !self.buf[self.cursor].is_whitespace() {
            self.cursor += 1;
        }
    }

    pub(super) fn delete_word(&mut self) {
        if self.delete_selection() {
            return;
        }
        let end = self.cursor;
        self.word_left();
        self.buf.drain(self.cursor..end);
    }

    pub(super) fn kill_to_start(&mut self) {
        if self.delete_selection() {
            return;
        }
        let start = self.line_start();
        self.buf.drain(start..self.cursor);
        self.cursor = start;
    }

    pub(super) fn kill_to_end(&mut self) {
        if self.delete_selection() {
            return;
        }
        let end = self.line_end();
        self.buf.drain(self.cursor..end);
    }

    pub fn token(&self) -> (usize, String) {
        let start = self.buf[..self.cursor]
            .iter()
            .rposition(|c| c.is_whitespace() || atom_index(*c).is_some())
            .map_or(0, |i| i + 1);
        (start, self.buf[start..self.cursor].iter().collect())
    }

    pub fn replace_token(&mut self, with: &str) {
        let (start, _) = self.token();
        let end = self.cursor;
        self.replace(start, end, with);
    }

    fn replace(&mut self, start: usize, end: usize, with: &str) {
        self.clear_selection();
        self.buf.splice(start..end, with.chars());
        self.cursor = start + with.chars().count();
    }
}
