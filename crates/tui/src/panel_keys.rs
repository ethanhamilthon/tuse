use crate::app::App;
use crossterm::event::KeyCode;

impl App {
    /// Esc closes the panel (an opened trace item goes back to the list); the scroll keys
    /// move it; Left/Right step through `/trace` items without wrapping.
    pub(crate) fn panel_key(&mut self, code: KeyCode, page: usize) -> bool {
        let step = self.palette.ui.scroll_step as isize;
        let page = page as isize;
        if self.panel.is_none() {
            return false;
        }
        match code {
            KeyCode::Esc => match self.trace_item {
                Some(at) => self.open_trace(Some(at)),
                None => self.panel = None,
            },
            KeyCode::Left | KeyCode::Right
                if self.editor.is_empty() && self.trace_item.is_some() =>
            {
                let at = self.trace_item.unwrap();
                let next = if code == KeyCode::Left {
                    at.saturating_sub(1)
                } else {
                    (at + 1).min(self.trace.len().saturating_sub(1))
                };
                self.open_trace_item(next);
            }
            KeyCode::Up if self.editor.is_empty() => self.panel.as_mut().unwrap().scroll_by(-step),
            KeyCode::Down if self.editor.is_empty() => self.panel.as_mut().unwrap().scroll_by(step),
            KeyCode::PageUp => self.panel.as_mut().unwrap().scroll_by(-page),
            KeyCode::PageDown => self.panel.as_mut().unwrap().scroll_by(page),
            _ => return false,
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn left_right_steps_through_trace_items_without_wrapping() {
        let mut app = crate::app::test_app("trace-nav");
        app.trace = vec![
            ("user", "hi".into()),
            ("agent", "hello".into()),
            ("python", "1 + 1".into()),
        ];
        app.open_trace_item(0);
        assert_eq!(app.panel.as_ref().unwrap().title, "Trace 1/3 user");

        // Left at start clamps to 0.
        assert!(app.panel_key(KeyCode::Left, 10));
        assert_eq!(app.trace_item, Some(0));

        // Right moves to 1, then 2, then clamps at 2.
        assert!(app.panel_key(KeyCode::Right, 10));
        assert_eq!(app.trace_item, Some(1));
        assert_eq!(app.panel.as_ref().unwrap().title, "Trace 2/3 agent");
        assert!(app.panel_key(KeyCode::Right, 10));
        assert_eq!(app.trace_item, Some(2));
        assert_eq!(app.panel.as_ref().unwrap().title, "Trace 3/3 python");
        assert!(app.panel_key(KeyCode::Right, 10));
        assert_eq!(app.trace_item, Some(2));

        // When editor is non-empty, Left/Right are not intercepted.
        app.paste("abc");
        assert!(!app.panel_key(KeyCode::Left, 10));
        assert_eq!(app.trace_item, Some(2));

        crate::app::cleanup(app);
    }
}
