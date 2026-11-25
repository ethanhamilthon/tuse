use crate::convo::record::Record;
use crate::session::bus::Signal;
use crate::session::view::history::{Update, decode, keep_replay};
use crate::session::{Session, State};
use std::sync::Arc;

impl Session {
    pub fn set_model(&self, model: &str, effort: &str) {
        let _ = self.update(
            Update {
                model: Some(model.into()),
                effort: Some(effort.into()),
                ..Update::default()
            },
            false,
        );
    }

    pub fn set_name(&self, title: &str) -> Result<(), String> {
        self.update(
            Update {
                title: Some(title.into()),
                ..Update::default()
            },
            false,
        )
        .map(|_| ())
    }

    pub(crate) fn set_session(self: &Arc<Self>, update: Update) -> Result<(), String> {
        let changed = self.update(update, true)?;
        self.hook(
            "session_updated",
            serde_json::json!({"event": "session_updated", "changed": changed}),
        );
        Ok(())
    }

    pub(crate) fn update(
        &self,
        update: Update,
        validate: bool,
    ) -> Result<Vec<&'static str>, String> {
        let mut history = update.history.map(decode).transpose()?;
        let mut st = self.state.lock().unwrap();
        let config = self.config();
        let cache = self.cached(&mut st);
        let mut meta = cache.meta.clone();
        let mut changed = vec![];
        let mut records = vec![];
        let check_effort = update.effort.is_some() || update.model.is_some();
        if let Some(title) = update.title {
            let title: String = title
                .trim()
                .replace(['\r', '\n'], " ")
                .chars()
                .take(80)
                .collect();
            if title.is_empty() {
                return Err("empty title".into());
            }
            if title != meta.title {
                meta.title = title.clone();
                changed.push("title");
                records.push(Record::Title { new: title });
            }
        }
        if let Some(model) = update.model {
            let (provider, name) = tuse_config::split_model(&model);
            if validate
                && (name.is_empty()
                    || model.chars().any(char::is_whitespace)
                    || config.connect(provider).is_none())
            {
                return Err(format!("model `{model}` is not configured"));
            }
            if model != meta.model {
                meta.model = model;
                changed.push("model");
            }
        }
        if let Some(effort) = update.effort
            && effort != meta.effort
        {
            meta.effort = effort;
            changed.push("effort");
        }
        if validate && check_effort {
            let (provider, _) = tuse_config::split_model(&meta.model);
            let supported = config
                .connect(provider)
                .map_or(&["default"][..], |c| crate::supported_efforts(c.api_type()));
            if !supported.contains(&meta.effort.as_str()) {
                return Err(format!(
                    "effort `{}` is not supported by `{}`",
                    meta.effort, meta.model
                ));
            }
        }
        if changed.contains(&"model") || changed.contains(&"effort") {
            records.push(Record::Settings {
                model: meta.model.clone(),
                effort: meta.effort.clone(),
            });
        }
        if let Some(rs) = &mut history {
            keep_replay(rs, &cache.records);
            let current = st.pending_history.as_deref().unwrap_or(&cache.records);
            if serde_json::to_vec(current).map_err(|e| e.to_string())?
                == serde_json::to_vec(rs).map_err(|e| e.to_string())?
            {
                history = None;
            } else {
                changed.push("history");
            }
        }
        let deferred = st.tool_step;
        let replacement = if deferred { None } else { history.as_deref() };
        if replacement.is_some() || !records.is_empty() {
            self.write_history(&mut st, Some(&meta), replacement, &records, &config)?;
        }
        if deferred {
            if history.is_some() {
                st.pending_history = history;
            }
        } else if replacement.is_some() {
            self.emit(Signal::HistoryReplaced);
        }
        if !records.is_empty() {
            self.emit(Signal::Usage);
        }
        Ok(changed)
    }

    pub(crate) fn end_tool_step(&self, st: &mut State) {
        st.tool_step = false;
        if let Some(history) = st.pending_history.take()
            && self
                .write_history(st, None, Some(&history), &[], &self.config())
                .is_ok()
        {
            self.emit(Signal::HistoryReplaced);
        }
    }
}
