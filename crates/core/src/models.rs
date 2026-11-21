use crate::event::Event;
use crate::session::Session;
use std::sync::Arc;
use tuse_api::Client;

/// Whether `full` (`provider/model`) has an active `connect` entry in `mode`.
pub fn model_available(config: &tuse_config::Config, mode: crate::Mode, full: &str) -> bool {
    let (provider, _) = tuse_config::split_model(full);
    !provider.is_empty() && config.connect(provider, mode.into()).is_some()
}

impl Session {
    /// Whether `full` (`provider/model`) can be used in this session's mode.
    pub fn model_available(&self, full: &str) -> bool {
        model_available(&self.config(), self.mode, full)
    }

    /// Models (`provider/model`) active providers have reported so far.
    pub fn remote_models(&self) -> Vec<String> {
        let config = self.config();
        self.state
            .lock()
            .unwrap()
            .remote
            .iter()
            .filter(|m| model_available(&config, self.mode, m))
            .cloned()
            .collect()
    }

    /// Configured and reported models whose provider is active in this session's mode.
    pub fn available_models(&self) -> Vec<String> {
        let config = self.config();
        let mut names: Vec<String> = config
            .models
            .iter()
            .map(|m| m.name.clone())
            .filter(|m| model_available(&config, self.mode, m))
            .collect();
        for m in self.remote_models() {
            if !names.contains(&m) {
                names.push(m);
            }
        }
        names.sort();
        names
    }

    /// Asks every `connect` provider for its models in the background.
    /// Each answer updates the cache and emits `Event::Models`.
    pub fn fetch_models(self: &Arc<Self>) {
        let mode = self.mode.into();
        for c in self
            .config()
            .connect
            .iter()
            .filter(|c| c.matches(mode))
            .cloned()
        {
            let s = self.clone();
            std::thread::spawn(
                move || match Client::new(&c.base_url, &c.api_key).models() {
                    Ok(ids) => {
                        let prefix = format!("{}/", c.name);
                        let mut st = s.state.lock().unwrap();
                        st.remote.retain(|m| !m.starts_with(&prefix));
                        st.remote
                            .extend(ids.into_iter().map(|id| format!("{prefix}{id}")));
                        drop(st);
                        s.emit(Event::Models);
                    }
                    Err(e) => s.emit(Event::Error(format!("{}: can't fetch models: {e}", c.name))),
                },
            );
        }
    }

    /// Writes a top-level key into the active config file and reloads it.
    /// Without a config file there is nothing to write to.
    pub fn save_setting(self: &Arc<Self>, key: &str, value: &str) -> Result<(), String> {
        self.save(|path| tuse_config::set_key(path, key, value))
    }

    /// Like `save_setting`, for a key inside `settings:`.
    pub fn save_in_settings(self: &Arc<Self>, key: &str, value: &str) -> Result<(), String> {
        self.save(|path| tuse_config::set_setting(path, key, value))
    }

    fn save(
        self: &Arc<Self>,
        write: impl Fn(&std::path::Path) -> Result<(), String>,
    ) -> Result<(), String> {
        match tuse_config::find(self.config_arg.as_deref(), &self.cwd) {
            Some(path) => {
                write(&path)?;
                self.reload()
            }
            None => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Mode;
    use tuse_config::{Config, Connect, Mode as CfgMode};

    #[test]
    fn model_availability_follows_active_connect_modes() {
        let conn = |name: &str, modes: Vec<CfgMode>| Connect {
            name: name.into(),
            base_url: "http://x".into(),
            api_key: "k".into(),
            modes,
        };
        let config = Config {
            connect: vec![
                conn("all", vec![]),
                conn("tui_only", vec![CfgMode::Tui]),
                conn("json_only", vec![CfgMode::Json]),
            ],
            ..Default::default()
        };
        assert!(model_available(&config, Mode::Tui, "all/m"));
        assert!(model_available(&config, Mode::Json, "all/org/m"));
        assert!(model_available(&config, Mode::Tui, "tui_only/m"));
        assert!(!model_available(&config, Mode::Json, "tui_only/m"));
        assert!(model_available(&config, Mode::Json, "json_only/m"));
        assert!(!model_available(&config, Mode::Headless, "json_only/m"));
        assert!(!model_available(&config, Mode::Tui, "missing/m"));
        assert!(!model_available(&config, Mode::Tui, "unprefixed"));
        assert!(!model_available(&config, Mode::Tui, ""));
    }
}
