use crate::session::Session;
use crate::session::bus::Signal;
use std::sync::Arc;
use tuse_api::{ApiType, Client};
use tuse_config::ModelDef;

pub fn model_available(config: &tuse_config::Config, full: &str) -> bool {
    let (provider, name) = tuse_config::split_model(full);
    !provider.is_empty()
        && !name.is_empty()
        && !full.chars().any(char::is_whitespace)
        && config.connect(provider).is_some()
}

impl Session {
    pub fn model_def(&self, full: &str) -> Option<ModelDef> {
        self.config().model_def(full).cloned()
    }

    pub fn display_name(&self, full: &str) -> String {
        tuse_config::display_name(self.model_def(full).as_ref(), full)
    }

    pub fn model_available(&self, full: &str) -> bool {
        model_available(&self.config(), full)
    }

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

    pub(crate) fn swap_config(&self, c: Arc<tuse_config::Config>) -> Result<(), String> {
        let mut st = self.state.lock().unwrap();
        self.rebuild_system(&mut st, &c)?;
        let mut cfg = self.config.write().unwrap();
        cfg.0 += 1;
        cfg.1 = c;
        st.remote.clear();
        Ok(())
    }

    pub fn fetch_models(self: &Arc<Self>) {
        let (config_gen, connect) = {
            let cfg = self.config.read().unwrap();
            (cfg.0, cfg.1.connect.clone())
        };
        for c in connect {
            let s = self.clone();
            std::thread::spawn(move || {
                match Client::new(&c.base_url, &c.api_key, ApiType::from_stored(c.api_type()))
                    .with_headers(c.headers)
                    .models()
                {
                    Ok(ids) => {
                        let mut st = s.state.lock().unwrap();
                        let cfg = s.config.read().unwrap();
                        if cfg.0 != config_gen {
                            return;
                        }
                        let prefix = format!("{}/", c.name);
                        st.remote.retain(|m| !m.starts_with(&prefix));
                        st.remote
                            .extend(ids.into_iter().map(|id| format!("{prefix}{id}")));
                        drop((st, cfg));
                        s.emit(Signal::Models);
                    }
                    Err(e) => {
                        let current = s.config.read().unwrap().0 == config_gen;
                        if current {
                            s.error(format!("{}: can't fetch models: {e}", c.name));
                        }
                    }
                }
            });
        }
    }

    pub fn select_model(
        self: &Arc<Self>,
        model: &str,
        effort: &str,
        persist: bool,
    ) -> Result<(), String> {
        let config = self.config();
        if model.is_empty() {
            if !effort.is_empty() && !tuse_config::DEFAULT_EFFORTS.contains(&effort) {
                return Err(format!(
                    "`effort` must be one of {}",
                    tuse_config::DEFAULT_EFFORTS.join(", ")
                ));
            }
        } else {
            if !self.model_available(model) {
                return Err(format!("Model `{model}` is unavailable"));
            }
            config.check_effort(model, effort)?;
        }
        self.update(
            crate::session::view::history::Update {
                model: Some(model.into()),
                effort: Some(effort.into()),
                ..Default::default()
            },
            false,
        )?;
        if persist {
            self.save_choice("model", model)?;
            self.save_choice("effort", effort)?;
        }
        Ok(())
    }

    pub fn save_choice(&self, key: &str, value: &str) -> Result<(), String> {
        crate::storage::Prefs::set(key, value)
    }
}
