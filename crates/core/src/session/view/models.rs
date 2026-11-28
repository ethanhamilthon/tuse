use crate::session::Session;
use crate::session::bus::Signal;
use crate::storage::{ConnectInput, ProviderPatch, ProviderResult, Providers};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::Arc;
use tuse_api::{ApiType, Client};
use tuse_config::ModelDef;

pub fn model_available(full: &str) -> bool {
    let (provider, name) = tuse_config::split_model(full);
    !provider.is_empty()
        && !name.is_empty()
        && !full.chars().any(char::is_whitespace)
        && Providers::has(provider)
}

impl Session {
    pub fn model_def(&self, full: &str) -> Option<ModelDef> {
        self.config().model_def(full).cloned()
    }

    pub fn display_name(&self, full: &str) -> String {
        tuse_config::display_name(self.model_def(full).as_ref(), full)
    }

    pub fn model_available(&self, full: &str) -> bool {
        model_available(full)
    }

    pub fn cached_model_available(&self, full: &str) -> bool {
        let (provider, name) = tuse_config::split_model(full);
        !provider.is_empty()
            && !name.is_empty()
            && !full.chars().any(char::is_whitespace)
            && self.state.lock().unwrap().provider_names.contains(provider)
    }

    pub fn remote_models(&self) -> Vec<String> {
        let st = self.state.lock().unwrap();
        st.remote
            .iter()
            .filter(|m| {
                let (provider, name) = tuse_config::split_model(m);
                !provider.is_empty()
                    && !name.is_empty()
                    && !m.chars().any(char::is_whitespace)
                    && st.provider_names.contains(provider)
            })
            .cloned()
            .collect()
    }

    pub fn available_models(&self) -> Vec<String> {
        let mut names = self.remote_models();
        names.sort();
        names.dedup();
        names
    }

    pub fn connect_provider(&self, input: ConnectInput) -> Result<ProviderResult, String> {
        let res = Providers::connect(input)?;
        self.update_provider_catalog(&res.name, Some(&res.model_ids));
        Ok(res)
    }

    pub fn update_provider(
        &self,
        name: &str,
        patch: ProviderPatch,
    ) -> Result<ProviderResult, String> {
        let res = Providers::update(name, patch)?;
        self.update_provider_catalog(&res.name, Some(&res.model_ids));
        Ok(res)
    }

    pub fn disconnect_provider(&self, name: &str) -> Result<String, String> {
        let removed = Providers::disconnect(name)?;
        self.update_provider_catalog(&removed, None);
        Ok(removed)
    }

    fn update_provider_catalog(&self, name: &str, models: Option<&[String]>) {
        let prefix = format!("{name}/");
        let mut st = self.state.lock().unwrap();
        st.discovery.remove(name);
        st.remote.retain(|m| !m.starts_with(&prefix));
        if let Some(ids) = models {
            st.provider_names.insert(name.into());
            st.remote
                .extend(ids.iter().map(|id| format!("{prefix}{id}")));
        } else {
            st.provider_names.remove(name);
        }
        drop(st);
        self.emit(Signal::Models);
    }

    pub(crate) fn swap_config(&self, c: Arc<tuse_config::Config>) -> Result<(), String> {
        let mut st = self.state.lock().unwrap();
        self.rebuild_system(&mut st, &c)?;
        self.config.write().unwrap().1 = c;
        Ok(())
    }

    pub fn fetch_models(self: &Arc<Self>) {
        let saved = match Providers::list() {
            Ok(list) => list,
            Err(e) => {
                self.error(format!("can't list providers: {e}"));
                return;
            }
        };
        let mut st = self.state.lock().unwrap();
        st.provider_names = saved.iter().map(|p| p.name.clone()).collect();
        st.remote.retain(|m| {
            saved
                .iter()
                .any(|p| p.name == tuse_config::split_model(m).0)
        });
        st.discovery
            .retain(|name, _| saved.iter().any(|p| &p.name == name));
        let started: Vec<_> = saved
            .into_iter()
            .filter_map(|p| {
                let mut hash = DefaultHasher::new();
                p.hash(&mut hash);
                let revision = hash.finish();
                if st
                    .discovery
                    .get(&p.name)
                    .is_some_and(|&(key, _)| key == revision)
                {
                    return None;
                }
                let id = self
                    .next_lua
                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                let job = (revision, id);
                st.discovery.insert(p.name.clone(), job);
                Some((p, job))
            })
            .collect();
        drop(st);
        for (raw, generation) in started {
            let s = self.clone();
            std::thread::spawn(move || {
                let result = raw.resolve().and_then(|c| {
                    Client::new(&c.base_url, &c.api_key, ApiType::from_stored(c.api_type()))
                        .with_headers(c.headers.clone())
                        .models()
                        .map_err(|e| {
                            crate::storage::providers::redact_secrets(&e.to_string(), &raw, &c)
                        })
                });
                let current = Providers::get(&raw.name).ok().flatten();
                let mut st = s.state.lock().unwrap();
                if st.discovery.get(&raw.name) != Some(&generation) {
                    return;
                }
                st.discovery.remove(&raw.name);
                if current.as_ref() != Some(&raw) {
                    return;
                }
                match result {
                    Ok(ids) => {
                        let prefix = format!("{}/", raw.name);
                        st.remote.retain(|m| !m.starts_with(&prefix));
                        st.remote
                            .extend(ids.into_iter().map(|id| format!("{prefix}{id}")));
                        drop(st);
                        s.emit(Signal::Models);
                    }
                    Err(e) => {
                        drop(st);
                        s.error(format!("{}: can't fetch models: {e}", raw.name));
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
