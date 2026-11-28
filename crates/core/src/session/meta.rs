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
        if !changed.is_empty() {
            self.hook(
                "session_updated",
                serde_json::json!({"event": "session_updated", "changed": changed}),
            );
        }
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
        let mut new_title = None;
        let check_effort = update.effort.is_some() || update.model.is_some();
        let effort_omitted = update.effort.is_none();
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
                new_title = Some(title);
            }
        }
        if let Some(model) = update.model {
            let (provider, name) = tuse_config::split_model(&model);
            if validate
                && (name.is_empty()
                    || model.chars().any(char::is_whitespace)
                    || !crate::storage::Providers::has(provider))
            {
                return Err(format!("model `{model}` is unavailable"));
            }
            if model != meta.model {
                meta.model = model;
                changed.push("model");
            }
            if validate && effort_omitted && config.check_effort(&meta.model, &meta.effort).is_err()
            {
                let next = config.fallback_effort(&meta.model);
                if next != meta.effort {
                    meta.effort = next;
                    changed.push("effort");
                }
            }
        }
        if let Some(effort) = update.effort
            && effort != meta.effort
        {
            meta.effort = effort;
            changed.push("effort");
        }
        if validate && check_effort {
            config.check_effort(&meta.model, &meta.effort)?;
        }
        if let Some(rs) = &history
            && let Some(crate::event::Switch::Fork(index)) = &st.pending_switch
        {
            crate::session::launch::check_fork(rs, *index)?;
        }
        let settings = changed.contains(&"model") || changed.contains(&"effort");
        if new_title.is_some() || settings {
            records.push(Record::Meta {
                title: new_title,
                model: settings.then(|| meta.model.clone()),
                effort: settings.then(|| meta.effort.clone()),
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
            if let Some(mut h) = history {
                h.extend(records.iter().cloned());
                st.pending_history = Some(h);
            } else if !records.is_empty()
                && let Some(pending) = &mut st.pending_history
            {
                pending.extend(records.iter().cloned());
            }
        } else if replacement.is_some() {
            self.emit(Signal::HistoryReplaced);
        }
        if !records.is_empty() {
            self.emit(Signal::Usage);
        }
        Ok(changed)
    }

    pub(crate) fn end_tool_step(&self, st: &mut State) -> bool {
        st.tool_step = false;
        if let Some(history) = st.pending_history.take()
            && self
                .write_history(st, None, Some(&history), &[], &self.config())
                .is_ok()
        {
            self.emit(Signal::HistoryReplaced);
        }
        match st.pending_switch.take() {
            Some(switch) => {
                self.emit(Signal::Switch(switch));
                true
            }
            None => false,
        }
    }
}
