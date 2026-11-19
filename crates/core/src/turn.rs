use crate::event::Event;
use crate::history::{estimate_tokens, messages};
use crate::hooks::Data;
use crate::record::Record;
use crate::session::{Session, State};
use crate::{compaction, id, prompt};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use tuse_api::{Client, Delta, Request};
use tuse_config::split_model;

enum Step {
    Continue,
    Done,
    Stale,
}

impl Session {
    pub(crate) fn turn(self: Arc<Self>, generation: u64, cancel: Arc<AtomicBool>) {
        self.hook("turn_start", Data::default());
        self.emit(Event::TurnStart);
        loop {
            match self.step(generation, &cancel) {
                Step::Continue => continue,
                Step::Stale => return,
                Step::Done if self.finish(generation) => return,
                Step::Done => continue,
            }
        }
    }

    /// Ends the turn unless messages arrived meanwhile. Returns false to keep going.
    fn finish(self: &Arc<Self>, generation: u64) -> bool {
        let mut st = self.state.lock().unwrap();
        if st.generation != generation {
            return true;
        }
        if !st.queue.is_empty() {
            return false;
        }
        st.busy = false;
        let (elapsed, calls) = (st.turn_start.elapsed(), st.calls);
        let r = Record::TurnEnd {
            elapsed_ms: elapsed.as_millis() as u64,
            calls,
            cancelled: false,
        };
        self.push(&mut st, r);
        self.save_meta(&mut st);
        drop(st);
        self.emit(Event::TurnEnd {
            elapsed,
            calls,
            cancelled: false,
        });
        self.hook("turn_end", Data::default());
        true
    }

    pub(crate) fn lock_live(&self, generation: u64) -> Option<std::sync::MutexGuard<'_, State>> {
        let st = self.state.lock().unwrap();
        (st.generation == generation).then_some(st)
    }

    fn step(self: &Arc<Self>, generation: u64, cancel: &AtomicBool) -> Step {
        let config = self.config();
        let Some(mut st) = self.lock_live(generation) else {
            return Step::Stale;
        };
        for (id, input) in std::mem::take(&mut st.queue) {
            if st.meta.title.is_empty() {
                st.meta.title = input
                    .text
                    .lines()
                    .next()
                    .unwrap_or("")
                    .chars()
                    .take(80)
                    .collect();
                self.save_meta(&mut st);
            }
            self.emit(Event::User {
                id,
                text: input.text.clone(),
                images: input.images.len(),
            });
            let r = Record::User {
                text: input.text,
                images: input.images,
                at: id::now(),
            };
            self.push(&mut st, r);
        }
        let system = prompt::system_prompt(&config, st.bounds.drop > 0);
        let (model, effort) = (st.model.clone(), st.effort.clone());
        let req = Request {
            model: split_model(&model).1.to_string(),
            messages: messages(&st.records, st.bounds, &system),
            tools: prompt::tools(&config),
            effort: (effort != "default").then_some(effort),
        };
        drop(st);
        let (provider, _) = split_model(&model);
        let Some(conn) = config.connect(provider) else {
            return self.fail(
                generation,
                format!("no `connect` entry named `{provider}` for model `{model}`"),
            );
        };
        let client = Client::new(&conn.base_url, &conn.api_key);
        let result = client.complete(
            &req,
            config.settings.retries,
            cancel,
            &mut |d| match d {
                Delta::Text(t) => self.emit(Event::Text(t.to_string())),
                Delta::Reasoning(t) => self.emit(Event::Reasoning(t.to_string())),
            },
            &mut |r| {
                self.emit(Event::StreamReset);
                self.emit(Event::Notice(format!(
                    "Retry {}/{} in {}s: {}",
                    r.attempt, r.max, r.delay, r.error
                )));
            },
        );
        let reply = match result {
            Err(tuse_api::Error::Cancelled) => return Step::Stale,
            Err(e) => return self.fail(generation, e.to_string()),
            Ok(r) => r,
        };
        let Some(mut st) = self.lock_live(generation) else {
            return Step::Stale;
        };
        if let Some(u) = reply.usage {
            st.context = u.input + u.output;
            self.push(
                &mut st,
                Record::Usage {
                    model: model.clone(),
                    usage: u,
                },
            );
        }
        let calls = reply.tool_calls.clone();
        let r = Record::Assistant {
            text: reply.text.clone(),
            reasoning: reply.reasoning,
            tool_calls: calls.clone(),
            at: id::now(),
        };
        self.push(&mut st, r);
        drop(st);
        self.emit(Event::Assistant { text: reply.text });
        self.emit(Event::Usage);
        if calls.is_empty() {
            return Step::Done;
        }
        for call in &calls {
            if !self.tool(generation, cancel, call, &config) {
                return Step::Stale;
            }
        }
        self.compact(generation, &config);
        Step::Continue
    }

    fn compact(&self, generation: u64, config: &tuse_config::Config) {
        let Some(mut st) = self.lock_live(generation) else {
            return;
        };
        let threshold = config.compact_at(&st.model);
        if threshold == 0 || st.context < threshold {
            return;
        }
        let Some(b) = compaction::bounds(&st.records, threshold, st.bounds) else {
            return;
        };
        st.bounds = b;
        self.push(
            &mut st,
            Record::Compaction {
                prune: b.prune,
                drop: b.drop,
            },
        );
        let system = prompt::system_prompt(config, true);
        st.context = estimate_tokens(&messages(&st.records, b, &system));
        let text = format!(
            "Context compacted at {} tokens; now about {}.",
            crate::usage::tokens(threshold),
            crate::usage::tokens(st.context)
        );
        self.push(&mut st, Record::Notice { text: text.clone() });
        drop(st);
        self.emit(Event::Notice(text));
    }

    fn fail(self: &Arc<Self>, generation: u64, text: String) -> Step {
        let Some(mut st) = self.lock_live(generation) else {
            return Step::Stale;
        };
        self.push(&mut st, Record::Error { text: text.clone() });
        drop(st);
        self.emit(Event::Error(text.clone()));
        self.hook(
            "error",
            Data {
                error: Some(text),
                ..Default::default()
            },
        );
        Step::Done
    }
}
