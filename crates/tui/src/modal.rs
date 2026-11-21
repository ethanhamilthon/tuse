use crate::app::App;
use crate::picker::Picker;

impl App {
    /// True while the editor is the search box of a modal picker.
    pub(crate) fn searching(&self) -> bool {
        self.picker
            .as_ref()
            .is_some_and(|p| !p.kind.is_token() && !p.is_free())
    }

    /// Opens a modal picker. Searchable ones take over the editor: the draft is
    /// set aside once and the query is typed into the (empty) editor.
    pub(crate) fn open_modal(&mut self, mut p: Picker) -> &mut Picker {
        if !p.is_free() {
            // A refresh of the same picker keeps what was typed.
            let same = self.picker.as_ref().is_some_and(|q| q.kind == p.kind);
            if self.saved_editor.is_none() {
                self.saved_editor = Some(std::mem::take(&mut self.editor));
            } else if !same {
                self.editor.clear();
            }
            p.filter(&self.editor.text());
        }
        self.picker.insert(p)
    }

    /// Closes the picker and gives the draft back to the editor.
    pub(crate) fn close_modal(&mut self) {
        self.picker = None;
        self.restore_draft();
        self.resume_ask();
    }

    pub(crate) fn restore_draft(&mut self) {
        if let Some(draft) = self.saved_editor.take() {
            self.editor = draft;
        }
    }
}

#[cfg(test)]
#[path = "modal_tests.rs"]
mod tests;
