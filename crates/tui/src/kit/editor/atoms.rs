use super::{ATOM_BASE, Editor};
use std::path::PathBuf;

pub struct Atom {
    pub label: String,
    pub kind: AtomKind,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AtomKind {
    Prompt(String, Option<String>),
    File(PathBuf),
    Image { id: usize, path: PathBuf },
}

impl Editor {
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

    fn active(&self) -> impl Iterator<Item = &AtomKind> {
        self.buf
            .iter()
            .filter_map(|c| self.atom(*c).map(|a| &a.kind))
    }

    pub(super) fn prompts(&self) -> Vec<String> {
        self.active()
            .filter_map(|k| match k {
                AtomKind::Prompt(n, None) => Some(n.clone()),
                AtomKind::Prompt(n, Some(a)) => Some(format!("{n} {a}")),
                _ => None,
            })
            .collect()
    }

    pub fn prompt_before_token(&self) -> Option<String> {
        let (start, _) = self.token();
        if start < 2 || self.buf[start - 1] != ' ' {
            return None;
        }
        match self.atom(self.buf[start - 2]).map(|a| &a.kind) {
            Some(AtomKind::Prompt(n, None)) => Some(n.clone()),
            _ => None,
        }
    }

    pub fn set_prompt_arg(&mut self, arg: &str) {
        let (start, _) = self.token();
        if self.prompt_before_token().is_none() {
            return;
        }
        let c = self.buf[start - 2];
        let end = self.cursor;
        self.buf.drain(start - 1..end);
        self.cursor = start - 1;
        if let Some(i) = super::atom_index(c)
            && let Some(atom) = self.atoms.get_mut(i)
            && let AtomKind::Prompt(n, _) = &atom.kind
        {
            atom.label = format!("#{n} {arg}");
            atom.kind = AtomKind::Prompt(n.clone(), Some(arg.to_string()));
        }
    }

    pub(super) fn files(&self) -> Vec<PathBuf> {
        self.active()
            .filter_map(|k| match k {
                AtomKind::File(p) => Some(p.clone()),
                _ => None,
            })
            .collect()
    }

    pub(super) fn images(&self) -> Vec<(usize, PathBuf)> {
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
    fn replacing_and_deleting_selections() {
        let mut e = Editor::default();
        e.insert("abc");
        e.insert_atom("@path", AtomKind::File("path".into()));
        e.select_all();
        assert_eq!(e.selected_text().as_deref(), Some("abc@path"));
        e.insert("x");
        assert_eq!(e.text(), "x");
        e.select_all();
        e.backspace();
        assert!(e.is_empty());
    }

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
        e.insert_atom("#p", AtomKind::Prompt("p".into(), None));
        e.insert_atom("#q", AtomKind::Prompt("q".into(), None));
        e.backspace();
        assert_eq!(e.prompts(), ["p"]);
        e.insert(" ap");
        assert_eq!(e.prompt_before_token().as_deref(), Some("p"));
        e.set_prompt_arg("apply");
        assert_eq!(e.prompts(), ["p apply"]);
        assert_eq!(e.text(), "#typed @typed @b.txt#p apply");
        e.insert(" ");
        assert_eq!(e.prompt_before_token(), None);
        assert_eq!(e.files(), [PathBuf::from("b.txt")]);
        assert!(e.images().is_empty());
    }
}
