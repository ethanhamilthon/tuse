use crate::app::App;
use crate::commands::COMMANDS;
use crate::picker::{Kind, Picker, item};

impl App {
    pub(crate) fn cut_token(&mut self) {
        self.replace_token("");
    }

    pub(crate) fn replace_token(&mut self, with: &str) {
        let (start, _) = self.editor.token();
        let end = self.editor.cursor;
        self.editor.replace(start, end, with);
    }

    /// Opens, filters or closes the `/`, `#` and `@` pickers for the token at the cursor.
    pub fn update_token_picker(&mut self) {
        if self.steering.is_some() || self.picker.as_ref().is_some_and(|p| !p.kind.is_token()) {
            return;
        }
        let (_, token) = self.editor.token();
        if self.dismissed.as_deref() == Some(&token) {
            return;
        }
        self.dismissed = None;
        let mut chars = token.chars();
        let (kind, query) = match chars.next() {
            Some('/') => (Kind::Command, chars.as_str()),
            Some('#')
                if self
                    .session
                    .config()
                    .prompts
                    .values()
                    .any(|p| p.matches(self.session.mode.into())) =>
            {
                (Kind::Prompt, chars.as_str())
            }
            Some('@') => (Kind::File, chars.as_str()),
            _ => {
                self.picker = None;
                return;
            }
        };
        let mut query = query.to_string();
        let mut dir = String::new();
        if kind == Kind::File {
            let (d, f) = crate::files::split(&query);
            (dir, query) = (d.to_string(), f.to_string());
        }
        let stale = kind == Kind::File && self.file_dir != dir;
        if stale || self.picker.as_ref().is_none_or(|p| p.kind != kind) {
            self.file_dir = dir;
            let items = self.token_items(&kind);
            self.picker = Some(Picker::new(kind, "", items));
        }
        if let Some(p) = &mut self.picker {
            p.filter(&query);
        }
    }

    fn token_items(&mut self, kind: &Kind) -> Vec<crate::picker::Item> {
        match kind {
            Kind::Command => COMMANDS
                .iter()
                .map(|(n, d)| item(format!("/{n}"), *d, *n))
                .collect(),
            Kind::Prompt => {
                let config = self.session.config();
                let mode = self.session.mode.into();
                config
                    .prompts
                    .iter()
                    .filter(|(_, v)| v.matches(mode))
                    .map(|(k, v)| {
                        item(
                            format!("#{k}"),
                            v.text().lines().next().unwrap_or(""),
                            k.clone(),
                        )
                    })
                    .collect()
            }
            _ => {
                let max = crate::style::FILE_PICKER_MAX;
                crate::files::list(&self.cwd, &self.file_dir, max)
                    .into_iter()
                    .map(|f| item(f.clone(), "", f))
                    .collect()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::editor::AtomKind;
    use crate::picker::Kind;

    #[test]
    fn prompt_picker_and_refs_filter_by_mode() {
        let mut app = crate::app::test_app("prompt-modes");
        std::fs::write(
            app.cwd.join("tuse.yaml"),
            "prompts:\n  hl: {text: only json, modes: [json]}\n",
        )
        .unwrap();
        app.session.reload().unwrap();
        app.editor.insert("#");
        app.update_token_picker();
        assert!(app.picker.is_none());

        std::fs::write(
            app.cwd.join("tuse.yaml"),
            r#"prompts:
  plain: plain body
  tui_only: {text: tui body, modes: [tui]}
  hl_only: {text: json body, modes: [json]}
"#,
        )
        .unwrap();
        app.session.reload().unwrap();
        app.update_token_picker();
        let p = app.picker.as_ref().unwrap();
        assert_eq!(p.kind, Kind::Prompt);
        let values: Vec<_> = p.items.iter().map(|i| i.value.as_str()).collect();
        assert_eq!(values, ["plain", "tui_only"]);

        app.editor.clear();
        for name in ["plain", "tui_only", "hl_only"] {
            app.editor
                .insert_atom(&format!("#{name}"), AtomKind::Prompt(name.into()));
        }
        assert_eq!(
            app.refs().prompts,
            vec![
                ("plain".into(), "plain body".into()),
                ("tui_only".into(), "tui body".into()),
            ]
        );
        crate::app::cleanup(app);
    }
}
