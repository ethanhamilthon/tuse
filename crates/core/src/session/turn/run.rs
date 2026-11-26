use crate::convo::record::Record;
use crate::convo::wire;
use crate::event::Event;
use crate::prompt;
use crate::session::Session;
use crate::session::bus::Signal;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use tuse_api::{ApiType, Client, Request};
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
            let auto_title = if self.cached(&mut st).meta.title.is_empty() {
                st.queue
                    .iter()
                    .find(|(_, i)| !i.notification)
                    .map(|(_, i)| i.text.lines().next().unwrap_or("").to_string())
            } else {
                None
            };
            drop(st);
            if let Some(title) = auto_title {
                let _ = self.set_name(&title);
            }
        }
        self.emit(Signal::TurnStart);
        loop {
            match self.step(generation, &cancel) {
                Step::Continue => continue,
                Step::Stale => return,
                Step::Done if self.finish(generation) => return,
                Step::Done => continue,
            }
        }
    }

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
        let cache = self.cached(&mut st);
        let (model, effort) = (cache.meta.model.clone(), cache.meta.effort.clone());
        let r = Record::TurnEnd {
            elapsed_ms: elapsed.as_millis() as u64,
            calls,
            model: model.clone(),
            effort: effort.clone(),
            cancelled: false,
        };
        self.record(
            &mut st,
            &[r],
            [Event::TurnEnd {
                elapsed,
                calls,
                model,
                effort,
                cancelled: false,
            }],
        );
        true
    }

    fn step(self: &Arc<Self>, generation: u64, cancel: &Arc<AtomicBool>) -> Step {
        let Some(mut st) = self.lock_live(generation) else {
            return Step::Stale;
        };
        st.partial.clear();
        st.reasoning.clear();
        let inputs = std::mem::take(&mut st.queue);
        for (id, input) in inputs {
            if input.notification {
                let event = Event::Notification(input.text.clone());
                self.record(
                    &mut st,
                    &[Record::Notification { text: input.text }],
                    [event],
                );
                continue;
            }
            let event = Event::User {
                id,
                text: input.text.clone(),
                images: input.images.len(),
            };
            let r = Record::User {
                text: input.text,
                context: input.context,
                images: input.images,
            };
            if self.record(&mut st, &[r], [event]) {
                self.simple_hook("user_message");
            }
        }
        let config = self.config();
        let cache = self.cached(&mut st);
        let model = cache.meta.model.clone();
        let effort = cache.meta.effort.clone();
        let (provider, raw_model) = split_model(&model);
        let req = Request {
            provider: provider.to_string(),
            model: raw_model.to_string(),
            system: wire::system(&cache.records),
            messages: wire::messages(&cache.records),
            tools: prompt::tools(&config),
            effort: (effort != "default").then_some(effort),
        };
        drop(st);
        let Some(conn) = config.connect(provider) else {
            return self.fail(
                generation,
                format!("no `connect` entry named `{provider}` for model `{model}`"),
            );
        };
        let client = Client::with_agent(
            self.http.clone(),
            &conn.base_url,
            &conn.api_key,
            ApiType::from_stored(conn.api_type()),
        )
        .with_headers(conn.headers.clone());
        let Some(mut st) = self.lock_live(generation) else {
            return Step::Stale;
        };
        st.streaming = true;
        drop(st);
        let result = client.complete_observed(
            &req,
            tuse_api::RETRY_DELAYS.len(),
            cancel,
            &mut |d| self.on_delta(generation, d),
            &mut |r| {
                if let Some(mut st) = self.lock_live(generation) {
                    self.set_activity(&mut st, crate::Activity::Retry);
                    st.partial.clear();
                    st.reasoning.clear();
                    let text =
                        format!("Retry {}/{} in {}s: {}", r.attempt, r.max, r.delay, r.error);
                    self.record(
                        &mut st,
                        &[Record::message(crate::Level::Flash, text.clone())],
                        [Event::StreamReset, Event::Flash(text)],
                    );
                }
            },
            &mut || {
                if let Some(mut st) = self.lock_live(generation) {
                    self.set_activity(&mut st, crate::Activity::Provider);
                }
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
        st.streaming = false;
        self.set_activity(&mut st, crate::Activity::Idle);
        let mut records = Vec::with_capacity(2);
        if let Some(usage) = reply.usage {
            records.push(Record::Usage {
                model,
                usage,
                llm: false,
            });
        }
        let calls = reply.tool_calls;
        records.push(Record::Agent {
            text: reply.text.clone(),
            reasoning: reply.reasoning,
            tool_calls: calls.clone(),
            replay: reply.replay,
            cancelled: false,
        });
        if !self.record(
            &mut st,
            &records,
            [Event::Assistant { text: reply.text }, Event::Usage],
        ) {
            return Step::Done;
        }
        st.partial.clear();
        st.reasoning.clear();
        st.tool_step = !calls.is_empty();
        drop(st);
        self.agent_message_hook(if calls.is_empty() {
            "finished"
        } else {
            "working"
        });
        if calls.is_empty() {
            return Step::Done;
        }
        for call in &calls {
            match self.tool(generation, cancel, call) {
                None => return Step::Stale,
                Some(true) => break,
                Some(false) => {}
            }
        }
        if let Some(mut st) = self.lock_live(generation)
            && self.end_tool_step(&mut st)
        {
            return Step::Done;
        }
        Step::Continue
    }

    fn fail(self: &Arc<Self>, generation: u64, text: String) -> Step {
        let Some(mut st) = self.lock_live(generation) else {
            return Step::Stale;
        };
        st.streaming = false;
        self.set_activity(&mut st, crate::Activity::Idle);
        let saved = self.record(
            &mut st,
            &[Record::message(crate::Level::Error, text.clone())],
            [Event::Error(text.clone())],
        );
        drop(st);
        if saved {
            self.hook(
                "turn_error",
                serde_json::json!({"event": "turn_error", "error": text}),
            );
        }
        Step::Done
    }
}
