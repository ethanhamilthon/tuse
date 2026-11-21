use crate::event::Event;
use crate::session::Session;
use std::sync::Arc;
use tuse_api::Client;

impl Session {
    /// Models (`provider/model`) the providers have reported so far.
    pub fn remote_models(&self) -> Vec<String> {
        self.state.lock().unwrap().remote.clone()
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
