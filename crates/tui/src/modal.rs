use crate::app::{App, QuestionPrompt};
use crate::panel::Panel;
use crate::picker::{Kind, Picker, item};

impl App {
    /// True while the editor is the search box of a modal picker.
    pub(crate) fn searching(&self) -> bool {
        self.picker.as_ref().is_some_and(|p| p.kind.is_searchable())
    }

    /// True while a Lua `show_picker` disables the input editor completely.
    pub(crate) fn input_disabled(&self) -> bool {
        self.picker
            .as_ref()
            .is_some_and(|p| matches!(p.kind, Kind::LuaPicker(_)))
    }

    /// True while a Lua `show_question` uses the input editor for the answer.
    pub(crate) fn answering(&self) -> bool {
        self.question.is_some()
    }

    pub(crate) fn visible_panel_mut(&mut self) -> Option<&mut Panel> {
        if self.picker.is_some() {
            return None;
        }
        self.question
            .as_mut()
            .map(|q| &mut q.panel)
            .or(self.panel.as_mut())
    }

    /// Opens a modal picker, saving the input draft once.
    pub(crate) fn open_modal(&mut self, mut p: Picker) -> &mut Picker {
        let same = self.picker.as_ref().is_some_and(|q| q.kind == p.kind);
        if self.saved_editor.is_none() {
            self.saved_editor = Some(std::mem::take(&mut self.editor));
        } else if !same {
            self.editor.clear();
        }
        if p.kind.is_searchable() {
            p.filter(&self.editor.text());
        }
        self.picker.insert(p)
    }

    pub(crate) fn open_lua_picker(&mut self, id: u64, options: Vec<String>) {
        self.question = None;
        self.panel = None;
        self.trace_back = None;
        self.trace_items.clear();
        let items = options
            .into_iter()
            .map(|s| item(s.clone(), "", s))
            .collect();
        self.open_modal(Picker::new(Kind::LuaPicker(id), items));
    }

    pub(crate) fn open_question(&mut self, id: u64, text: String) {
        self.picker = None;
        self.panel = None;
        self.trace_back = None;
        self.trace_items.clear();
        if self.saved_editor.is_none() {
            self.saved_editor = Some(std::mem::take(&mut self.editor));
        } else {
            self.editor.clear();
        }
        self.question = Some(QuestionPrompt {
            id,
            panel: Panel::markdown(text),
        });
    }

    /// Closes the picker and gives the draft back to the editor.
    pub(crate) fn close_modal(&mut self) {
        self.picker = None;
        self.restore_draft();
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
