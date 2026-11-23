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

    /// Replaces the active config, bumps its generation, and clears cached remote models.
    pub(crate) fn swap_config(&self, c: Arc<tuse_config::Config>) {
        let mut cfg = self.config.write().unwrap();
        cfg.0 += 1;
        cfg.1 = c;
        self.state.lock().unwrap().remote.clear();
    }

    /// Asks every `connect` provider for its models in the background.
    /// Each answer updates the cache and emits `Event::Models`; stale generations are discarded.
    pub fn fetch_models(self: &Arc<Self>) {
        let mode = self.mode.into();
        let (config_gen, connect) = {
            let cfg = self.config.read().unwrap();
            let list: Vec<_> = cfg
                .1
                .connect
                .iter()
                .filter(|c| c.matches(mode))
                .cloned()
                .collect();
            (cfg.0, list)
        };
        for c in connect {
            let s = self.clone();
            std::thread::spawn(
                move || match Client::new(&c.base_url, &c.api_key).models() {
                    Ok(ids) => {
                        let cfg = s.config.read().unwrap();
                        if cfg.0 != config_gen {
                            return;
                        }
                        let prefix = format!("{}/", c.name);
                        let mut st = s.state.lock().unwrap();
                        st.remote.retain(|m| !m.starts_with(&prefix));
                        st.remote
                            .extend(ids.into_iter().map(|id| format!("{prefix}{id}")));
                        drop((st, cfg));
                        s.emit(Event::Models);
                    }
                    Err(e) => {
                        if s.config.read().unwrap().0 == config_gen {
                            s.emit(Event::Error(format!("{}: can't fetch models: {e}", c.name)));
                        }
                    }
                },
            );
        }
    }

    /// Remembers a choice of the UI (`model` or `effort`) in the database and
    /// applies it to the config at once; new sessions start with it.
    pub fn save_choice(&self, key: &str, value: &str) -> Result<(), String> {
        crate::db::set_state(key, value)?;
        let mut c = (*self.config()).clone();
        crate::store::apply_state(&mut c);
        self.swap_config(Arc::new(c));
        Ok(())
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
        assert!(!model_available(&config, Mode::Tui, "json_only/m"));
        assert!(!model_available(&config, Mode::Tui, "missing/m"));
        assert!(!model_available(&config, Mode::Tui, "unprefixed"));
        assert!(!model_available(&config, Mode::Tui, ""));
    }
}
