use super::Editor;
impl Editor {
    pub(super) fn selection_range(&self) -> Option<(usize, usize)> {
        self.anchor
            .filter(|a| *a != self.cursor)
            .map(|a| (a.min(self.cursor), a.max(self.cursor)))
    }
    pub fn selected_text(&self) -> Option<String> {
        let (a, b) = self.selection_range()?;
        Some(self.range_text(a, b))
    }
    pub(super) fn delete_selection(&mut self) -> bool {
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
    pub(super) fn select_all(&mut self) {
        self.anchor = Some(0);
        self.cursor = self.buf.len();
    }
    pub(super) fn clear_selection(&mut self) {
        self.anchor = None;
    }
}
