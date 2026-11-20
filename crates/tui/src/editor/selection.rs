use super::Editor;
impl Editor {
    pub fn selection_range(&self) -> Option<(usize, usize)> {
        self.anchor
            .filter(|a| *a != self.cursor)
            .map(|a| (a.min(self.cursor), a.max(self.cursor)))
    }
    pub fn selected_text(&self) -> Option<String> {
        let (a, b) = self.selection_range()?;
        Some(self.range_text(a, b))
    }
    pub fn delete_selection(&mut self) -> bool {
        let range = self.selection_range();
        self.clear_selection();
        if let Some((a, b)) = range {
            self.buf.drain(a..b);
            self.cursor = a;
            true
        } else {
            false
        }
    }
    pub fn select_all(&mut self) {
        self.anchor = Some(0);
        self.cursor = self.buf.len();
    }
    pub fn clear_selection(&mut self) {
        self.anchor = None;
    }
}
#[test]
fn replacing_and_deleting_selections() {
    let mut e = Editor::default();
    e.insert("abc");
    e.insert_atom("@path", super::AtomKind::File("path".into()));
    e.select_all();
    assert_eq!(e.selected_text().as_deref(), Some("abc@path"));
    e.insert("x");
    assert_eq!(e.text(), "x");
    e.select_all();
    e.backspace();
    assert!(e.is_empty());
}
