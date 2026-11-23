use crate::event::Event;
use crate::history::{estimate_tokens, messages};
use crate::record::Record;
use crate::session::{Session, State};
use crate::{compaction, id, prompt};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use tuse_api::{Client, Request};
use tuse_config::split_model;

enum Step {
    Continue,
    Done,
    Stale,
}

impl Session {
    pub(crate) fn turn(self: Arc<Self>, generation: u64, cancel: Arc<AtomicBool>) {
        {
            let Some(mut st) = self.lock_live(generation) else {
                return;
            };
            let input = st
                .queue
                .iter()
                .find(|(_, i)| !i.notification)
                .map(|(_, i)| i.text.clone());
            if st.meta.title.is_empty()
                && let Some(input) = &input
            {
                st.meta.title = input
                    .lines()
                    .next()
                    .unwrap_or("")
                    .chars()
                    .take(80)
                    .collect();
                self.save_meta(&mut st);
            }
            st.turn_error = None;
        }
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
        st.turn_error = None;
        st.busy = false;
        let (elapsed, calls) = (st.turn_start.elapsed(), st.calls);
        let (model, effort) = (st.model.clone(), st.effort.clone());
        let r = Record::TurnEnd {
            elapsed_ms: elapsed.as_millis() as u64,
            calls,
            model: model.clone(),
            effort: effort.clone(),
            cancelled: false,
        };
        self.push(&mut st, r);
        self.save_meta(&mut st);
        drop(st);
        self.emit(Event::TurnEnd {
            elapsed,
            calls,
            model,
            effort,
            cancelled: false,
        });
        true
    }

    pub(crate) fn lock_live(&self, generation: u64) -> Option<std::sync::MutexGuard<'_, State>> {
        let st = self.state.lock().unwrap();
        (st.generation == generation).then_some(st)
    }

    fn step(self: &Arc<Self>, generation: u64, cancel: &Arc<AtomicBool>) -> Step {
        let config = self.config();
        let Some(mut st) = self.lock_live(generation) else {
            return Step::Stale;
        };
        st.partial.clear();
        let mut typed = vec![];
        for (id, input) in std::mem::take(&mut st.queue) {
            if input.notification {
                self.emit(Event::Notification(input.text.clone()));
                self.push(&mut st, Record::Notification { text: input.text });
                continue;
            }
            self.emit(Event::User {
                id,
                text: input.text.clone(),
                images: input.images.len(),
            });
            let is_first = !st.records.iter().any(|r| matches!(r, Record::User { .. }));
            typed.push((input.text.clone(), is_first));
            let r = Record::User {
                text: input.text,
                context: input.context,
                images: input.images,
                at: id::now(),
            };
            self.push(&mut st, r);
        }
        let system = prompt::system_prompt(&config, st.bounds.drop > 0);
        let (model, effort) = (st.model.clone(), st.effort.clone());
        drop(st);
        for (text, is_first) in typed {
            self.user_message_hook(text, is_first);
        }
        let Some(st) = self.lock_live(generation) else {
            return Step::Stale;
        };
        let req = Request {
            model: split_model(&model).1.to_string(),
            messages: messages(&st.records, st.bounds, &system),
            tools: prompt::tools(),
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
        let client = Client::with_agent(self.http.clone(), &conn.base_url, &conn.api_key);
        let result = client.complete(
            &req,
            config.settings.retries,
            cancel,
            &mut |d| self.on_delta(d),
            &mut |r| {
                self.emit(Event::StreamReset);
                self.emit(Event::Flash(format!(
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
        st.partial.clear();
        drop(st);
        if !reply.text.trim().is_empty() {
            let status = if calls.is_empty() {
                "finished"
            } else {
                "working"
            };
            self.agent_message_hook(reply.text.clone(), status);
        }
        self.emit(Event::Assistant { text: reply.text });
        self.emit(Event::Usage);
        if calls.is_empty() {
            return Step::Done;
        }
        for call in &calls {
            if !self.tool(generation, cancel, call) {
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
        st.turn_error = Some(text.clone());
        self.push(&mut st, Record::Error { text: text.clone() });
        drop(st);
        self.emit(Event::Error(text));
        Step::Done
    }
}
