use crate::app::App;
use crate::picker::{Item, Kind, Picker, item};
use tuse_config::ModelDef;

impl App {
    /// Every available model providers reported plus configured models.
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
            .map(|n| {
                let def = config.model_def(n);
                model_item(def, n, self.session.model_available(n))
            })
            .collect();
        let selected = self
            .picker
            .as_ref()
            .filter(|p| p.kind == Kind::Model)
            .and_then(|p| p.current())
            .map_or(current, |i| i.value.clone());
        // A refresh keeps the search typed in the editor and re-applies it.
        self.open_modal(Picker::new(Kind::Model, items))
            .select(&selected);
    }
}

fn model_item(def: Option<&ModelDef>, name: &str, available: bool) -> Item {
    let shown = tuse_config::display_name(def, name);
    let mut parts = if shown == name {
        vec![]
    } else {
        vec![name.to_string()]
    };
    if !available && !name.is_empty() {
        parts.push("unavailable".to_string());
    }
    if let Some(m) = def {
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
        out.push(format!("${}/${} per 1M", p.input, p.output));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_detail_repeats_the_name_only_under_a_display_name() {
        let opus = ModelDef {
            name: "c/opus".into(),
            window: 200_000,
            ..Default::default()
        };
        let sonnet = ModelDef {
            name: "c/sonnet".into(),
            display: "Sonnet".into(),
            window: 200_000,
            ..Default::default()
        };
        let plain = model_item(Some(&opus), "c/opus", true);
        assert_eq!(
            (plain.label.as_str(), plain.detail.as_str()),
            ("c/opus", "200.0k ctx")
        );
        let named = model_item(Some(&sonnet), "c/sonnet", true);
        assert_eq!(named.detail, "c/sonnet · 200.0k ctx");
        assert_eq!(model_item(None, "c/other", true).detail, "");
        assert_eq!(
            model_item(Some(&opus), "c/opus", false).detail,
            "unavailable · 200.0k ctx"
        );
    }
}
