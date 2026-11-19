use crate::app::App;
use crate::picker::{Item, Kind, Picker, item};
use crate::style::Palette;
use tuse_config::{Config, ModelDef};

impl App {
    /// Every model the providers reported plus the ones named in `models:`.
    /// A `models:` entry only adds details (name, window, prices).
    pub fn open_models(&mut self) {
        let config = self.session.config();
        let (current, _) = self.session.model();
        let mut names = self.session.remote_models();
        for m in &config.models {
            if !names.contains(&m.name) {
                names.push(m.name.clone());
            }
        }
        if !names.contains(&current) {
            names.insert(0, current.clone());
        }
        names.sort();
        let items: Vec<Item> = names.iter().map(|n| model_item(&config, n)).collect();
        let selected = self
            .picker
            .as_ref()
            .filter(|p| p.kind == Kind::Model)
            .and_then(|p| p.current())
            .map_or(current, |i| i.value.clone());
        let mut p = Picker::new(Kind::Model, "Model", items);
        p.select(&selected);
        self.picker = Some(p);
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
        let mut p = Picker::new(Kind::Theme, "Theme", items);
        let active = match config.theme_name() {
            "" => "default",
            name => name,
        };
        p.select(active);
        self.picker = Some(p);
    }

    pub fn set_theme(&mut self, name: &str) {
        if let Err(e) = self.session.save_in_settings("theme", name) {
            self.error(format!("Can't save the theme: {e}"));
            return;
        }
        self.palette = Palette::new(&self.session.config().active_theme());
    }
}

fn model_item(config: &Config, name: &str) -> Item {
    let mut parts = vec![name.to_string()];
    if let Some(m) = config.model_def(name) {
        parts.extend(details(m));
    }
    item(config.display_name(name), parts.join(" · "), name)
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
