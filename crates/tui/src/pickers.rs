use crate::app::App;
use crate::picker::{Item, Kind, Picker, item};
use tuse_config::{Config, ModelDef};

impl App {
    /// Every available model the providers reported plus the ones named in `models:`.
    /// The current model stays visible even when its provider has been removed.
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
}
