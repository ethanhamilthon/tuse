use super::{ATOM_BASE, Editor};
use std::path::PathBuf;

/// A file, prompt or image in the input. The buffer holds one placeholder
/// char for it, so deleting and the arrows treat it as a single letter.
pub struct Atom {
    pub label: String,
    pub kind: AtomKind,
}

/// What an atom stands for.
#[derive(Debug, Clone, PartialEq)]
pub enum AtomKind {
    Prompt(String),
    File(PathBuf),
    Image { id: usize, path: PathBuf },
}

impl Editor {
    /// Inserts `[Image #N]`; `N` never repeats within one input.
    pub fn insert_image(&mut self, path: PathBuf) {
        let id = 1 + self
            .atoms
            .iter()
            .filter(|a| matches!(a.kind, AtomKind::Image { .. }))
            .count();
        self.insert_atom(&format!("[Image #{id}]"), AtomKind::Image { id, path });
    }

    pub fn insert_atom(&mut self, label: &str, kind: AtomKind) {
        self.delete_selection();
        let c = char::from_u32(ATOM_BASE + self.atoms.len() as u32).unwrap_or('?');
        self.atoms.push(Atom {
            label: label.to_string(),
            kind,
        });
        self.buf.insert(self.cursor, c);
        self.cursor += 1;
    }

    /// The atoms still in the buffer, in order of appearance.
    fn active(&self) -> impl Iterator<Item = &AtomKind> {
        self.buf
            .iter()
            .filter_map(|c| self.atom(*c).map(|a| &a.kind))
    }

    /// Names of the prompts picked with `#`.
    pub fn prompts(&self) -> Vec<String> {
        self.active()
            .filter_map(|k| match k {
                AtomKind::Prompt(n) => Some(n.clone()),
                _ => None,
            })
            .collect()
    }

    /// Files picked with `@`.
    pub fn files(&self) -> Vec<PathBuf> {
        self.active()
            .filter_map(|k| match k {
                AtomKind::File(p) => Some(p.clone()),
                _ => None,
            })
            .collect()
    }

    /// Pasted images as `(N, path)`.
    pub fn images(&self) -> Vec<(usize, PathBuf)> {
        self.active()
            .filter_map(|k| match k {
                AtomKind::Image { id, path } => Some((*id, path.clone())),
                _ => None,
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atom_is_one_letter() {
        let mut e = Editor::default();
        e.insert("a ");
        e.insert_atom("@src/main.rs", AtomKind::File("src/main.rs".into()));
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
        e.insert_image("/y.png".into());
        assert_eq!(e.text(), "[Image #1][Image #2]");
        e.backspace();
        e.insert_image("/z.png".into());
        assert_eq!(e.text(), "[Image #1][Image #3]");
        assert_eq!(
            e.images(),
            [(1, PathBuf::from("/x.png")), (3, PathBuf::from("/z.png"))]
        );
        e.backspace();
        assert_eq!(e.images(), [(1, PathBuf::from("/x.png"))]);
        assert_eq!(e.token().1, "");
    }

    #[test]
    fn atoms_are_listed_in_order_and_typed_tokens_are_not_atoms() {
        let mut e = Editor::default();
        e.insert("#typed @typed ");
        e.insert_atom("@b.txt", AtomKind::File("b.txt".into()));
        e.insert_atom("#p", AtomKind::Prompt("p".into()));
        e.insert_atom("#q", AtomKind::Prompt("q".into()));
        e.backspace();
        assert_eq!(e.prompts(), ["p"]);
        assert_eq!(e.files(), [PathBuf::from("b.txt")]);
        assert!(e.images().is_empty());
        assert_eq!(e.text(), "#typed @typed @b.txt#p");
    }
}
