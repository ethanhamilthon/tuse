use crate::convo::record::Record;
use crate::event::Event;
use crate::session::Session;
use std::sync::Arc;

impl Session {
    pub fn set_model(self: &Arc<Self>, model: &str, effort: &str) {
        let mut st = self.state.lock().unwrap();
        let (prev_model, prev_effort) = (st.model.clone(), st.effort.clone());
        let changed = prev_model != model || prev_effort != effort;
        st.model = model.to_string();
        st.effort = effort.to_string();
        let r = Record::Settings {
            model: model.into(),
            effort: effort.into(),
        };
        self.push(&mut st, r);
        self.save_meta(&mut st);
        drop(st);
        self.emit(Event::Usage);
        if changed {
            self.model_change_hook(&prev_model, &prev_effort, model, effort);
        }
    }

    pub fn set_name(self: &Arc<Self>, text: &str) -> Result<(), String> {
        let text: String = text
            .trim()
            .replace(['\r', '\n'], " ")
            .chars()
            .take(80)
            .collect();
        if text.is_empty() {
            return Err("empty title".into());
        }
        let mut st = self.state.lock().unwrap();
        let prev = st.meta.title.clone();
        if prev == text {
            return Ok(());
        }
        st.meta.title = text.clone();
        self.save_meta(&mut st);
        drop(st);
        self.name_change_hook((!prev.is_empty()).then_some(prev), text);
        Ok(())
    }
}
