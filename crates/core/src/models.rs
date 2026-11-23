use crate::event::Event;
use crate::session::Session;
use std::sync::Arc;
use tuse_api::Client;
use tuse_config::ModelDef;

/// Whether `full` (`provider/model`) has an active `connect` entry.
pub fn model_available(config: &tuse_config::Config, full: &str) -> bool {
    let (provider, _) = tuse_config::split_model(full);
    !provider.is_empty() && config.connect(provider).is_some()
}

impl Session {
    /// Configured metadata for `full` (`provider/model`).
    pub fn model_def(&self, full: &str) -> Option<ModelDef> {
        self.config().model_def(full).cloned()
    }

    pub fn display_name(&self, full: &str) -> String {
        tuse_config::display_name(self.model_def(full).as_ref(), full)
    }

    /// Whether `full` (`provider/model`) can be used in this session.
    pub fn model_available(&self, full: &str) -> bool {
        model_available(&self.config(), full)
    }

    /// Models (`provider/model`) active providers have reported so far.
    pub fn remote_models(&self) -> Vec<String> {
        let config = self.config();
        self.state
            .lock()
            .unwrap()
            .remote
            .iter()
            .filter(|m| model_available(&config, m))
            .cloned()
            .collect()
    }

    /// Configured and reported models whose provider is active in this session.
    pub fn available_models(&self) -> Vec<String> {
        let config = self.config();
        let mut names: Vec<String> = config
            .models
            .iter()
            .map(|m| m.name.clone())
            .filter(|m| model_available(&config, m))
            .collect();
        for m in self.remote_models() {
            if !names.contains(&m) {
                names.push(m);
            }
        }
        names.sort();
        names.dedup();
        names
    }

    /// Replaces the active config, bumps its generation, and clears cached remote models.
    pub(crate) fn swap_config(&self, c: Arc<tuse_config::Config>) {
        let mut cfg = self.config.write().unwrap();
        cfg.0 += 1;
        let mut st = self.state.lock().unwrap();
        st.remote.clear();
        if st.stats.1 != c.models {
            st.stats = (
                crate::usage::Stats::of(&st.records, &c.models),
                c.models.clone(),
            );
        }
        cfg.1 = c;
    }

    /// Asks every `connect` provider for its models in the background.
    /// Each answer updates the cache and emits `Event::Models`; stale generations are discarded.
    pub fn fetch_models(self: &Arc<Self>) {
        let (config_gen, connect) = {
            let cfg = self.config.read().unwrap();
            (cfg.0, cfg.1.connect.clone())
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

    /// Validates and sets the session's model and effort, optionally persisting both as defaults.
    pub fn select_model(
        self: &Arc<Self>,
        model: &str,
        effort: &str,
        persist: bool,
    ) -> Result<(), String> {
        if !tuse_config::EFFORTS.contains(&effort) {
            return Err(format!(
                "`effort` must be one of {}",
                tuse_config::EFFORTS.join(", ")
            ));
        }
        if !model.is_empty() && !self.model_available(model) {
            return Err(format!("Model `{model}` is unavailable"));
        }
        self.set_model(model, effort);
        if persist {
            self.save_choice("model", model)?;
            self.save_choice("effort", effort)?;
        }
        Ok(())
    }

    /// Remembers a default choice (`model` or `effort`) in SQLite; new sessions start with it.
    pub fn save_choice(&self, key: &str, value: &str) -> Result<(), String> {
        crate::db::set_state(key, value)
    }
}
