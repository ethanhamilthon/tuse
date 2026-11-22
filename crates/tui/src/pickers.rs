use crate::app::App;
use crate::picker::{Item, Kind, Picker, item};
use crate::style::Palette;
use tuse_config::{Config, ModelDef};

impl App {
    /// Every available model the providers reported plus the ones named in `models:`.
    /// A saved/configured current model whose provider is excluded stays visible marked unavailable.
    pub fn open_models(&mut self) {
        let config = self.session.config();
        let (current, _) = self.session.model();
        let mut names = self.session.available_models();
        if !names.contains(&current) {
            names.insert(0, current.clone());
        }
        names.sort();
        let items: Vec<Item> = names
            .iter()
            .map(|n| model_item(&config, n, self.session.model_available(n)))
            .collect();
        let selected = self
            .picker
            .as_ref()
            .filter(|p| p.kind == Kind::Model)
            .and_then(|p| p.current())
            .map_or(current, |i| i.value.clone());
        // A refresh keeps the search typed in the editor and re-applies it.
        self.open_modal(Picker::new(Kind::Model, "Model", items))
            .select(&selected);
        if self.session.remote_models().is_empty() {
            self.session.fetch_models();
        }
    }

    pub fn open_themes(&mut self) {
        let config = self.session.config();
        let mut names = vec!["default".to_string()];
        names.extend(config.themes.keys().filter(|k| *k != "default").cloned());
        let items = names
            .iter()
            .map(|n| item(n.clone(), "", n.clone()))
            .collect();
        let active = match config.theme_name() {
            "" => "default",
            name => name,
        };
        self.open_modal(Picker::new(Kind::Theme, "Theme", items))
            .select(active);
    }

    pub fn set_theme(&mut self, name: &str) {
        if let Err(e) = self.session.save_choice("theme", name) {
            self.error(format!("Can't save the theme: {e}"));
            return;
        }
        self.palette = Palette::from_config(&self.session.config());
        self.palette_gen += 1;
        self.chat.invalidate();
        if let Some(panel) = &mut self.panel {
            panel.invalidate();
        }
    }
}

fn model_item(config: &Config, name: &str, available: bool) -> Item {
    let shown = config.display_name(name);
    let mut parts = if shown == name {
        vec![]
    } else {
        vec![name.to_string()]
    };
    if !available && !name.is_empty() {
        parts.push("unavailable".to_string());
    }
    if let Some(m) = config.model_def(name) {
        parts.extend(details(m));
    }
    item(shown, parts.join(" · "), name)
}

fn details(m: &ModelDef) -> Vec<String> {
    let mut out = vec![];
    if m.window > 0 {
        out.push(format!("{} ctx", tuse_core::tokens(m.window)));
    }
    if let Some(p) = &m.prices {
        out.push(format!("${}/${} per 1M", p.input, p.out));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_detail_repeats_the_name_only_under_a_display_name() {
        let def = |name: &str, display: &str| ModelDef {
            name: name.into(),
            display: display.into(),
            window: 200_000,
            ..Default::default()
        };
        let config = Config {
            models: vec![def("c/opus", ""), def("c/sonnet", "Sonnet")],
            ..Default::default()
        };
        let plain = model_item(&config, "c/opus", true);
        assert_eq!(
            (plain.label.as_str(), plain.detail.as_str()),
            ("c/opus", "200.0k ctx")
        );
        let named = model_item(&config, "c/sonnet", true);
        assert_eq!(named.detail, "c/sonnet · 200.0k ctx");
        assert_eq!(model_item(&config, "c/other", true).detail, "");
        assert_eq!(
            model_item(&config, "c/opus", false).detail,
            "unavailable · 200.0k ctx"
        );
    }

    #[test]
    fn model_picker_hides_excluded_providers_and_marks_unavailable_current() {
        let mut app = crate::app::test_app("picker-modes");
        for (name, mode) in [
            ("tui_p", tuse_config::Mode::Tui),
            ("json_p", tuse_config::Mode::Json),
        ] {
            tuse_core::add_provider(&tuse_config::Connect {
                name: name.into(),
                base_url: "http://127.0.0.1:1".into(),
                api_key: "k".into(),
                modes: vec![mode],
            })
            .unwrap();
        }
        std::fs::write(
            app.cwd.join("tuse.lua"),
            "return { models = { {name = 'tui_p/a'}, {name = 'json_p/b'}, {name = 'json_p/saved'} } }",
        )
        .unwrap();
        app.session.reload().unwrap();
        app.session.set_model("json_p/saved", "default");
        app.paste("keep me");
        app.open_models();
        let p = app.picker.as_ref().unwrap();
        let items: Vec<_> = p
            .items
            .iter()
            .map(|i| (i.value.as_str(), i.detail.as_str()))
            .collect();
        assert_eq!(items, [("json_p/saved", "unavailable"), ("tui_p/a", "")]);

        // Selecting the unavailable current model is refused and restores the draft.
        app.choose();
        assert!(app.picker.is_none());
        assert_eq!(app.editor.text(), "keep me");
        crate::app::cleanup(app);
    }
}
