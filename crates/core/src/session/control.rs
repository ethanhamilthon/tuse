use crate::convo::record::{Record, UserInput};
use crate::convo::wire;
use crate::event::{Event, InputId};
use crate::session::Session;
use crate::session::bus::Signal;
use std::sync::atomic::Ordering;
use std::sync::{Arc, atomic::AtomicBool};
use std::time::Instant;

impl Session {
    pub fn submit(self: &Arc<Self>, input: UserInput) -> (InputId, bool) {
        let mut st = self.state.lock().unwrap();
        st.next_input += 1;
        let id = InputId(st.next_input);
        if !input.notification
            && let Some(request) = &st.tool_handoff
        {
            request.store(true, Ordering::Release);
        }
        st.queue.push((id, input));
        if st.busy || self.dir.lock().unwrap().is_none() {
            return (id, st.busy);
        }
        st.busy = true;
        st.generation += 1;
        st.cancel = Arc::new(AtomicBool::new(false));
        st.turn_start = Instant::now();
        st.calls = 0;
        st.status = String::new();
        let (generation, cancel) = (st.generation, st.cancel.clone());
        drop(st);
        let s = self.clone();
        std::thread::spawn(move || s.turn(generation, cancel));
        (id, false)
    }

    pub fn withdraw_input(&self, id: InputId) -> bool {
        let mut st = self.state.lock().unwrap();
        let Some(i) = st.queue.iter().position(|(queued, _)| *queued == id) else {
            return false;
        };
        st.queue.remove(i);
        true
    }

    pub fn cancel(self: &Arc<Self>) {
        self.stop_commands();
        self.stop_hook_runs();
        self.ui_cancel_all();
        self.cancel_turn(false);
        self.tasks.stop_all();
        self.emit(Signal::Tasks);
    }

    pub(crate) fn cancel_turn(self: &Arc<Self>, handoff: bool) {
        let mut st = self.state.lock().unwrap();
        if !st.busy {
            return;
        }
        if handoff && let Some(request) = &st.tool_handoff {
            request.store(true, Ordering::Release);
            st.cancel_pending = true;
            return;
        }
        self.finish_cancel(st);
    }

    pub(crate) fn finish_cancel(self: &Arc<Self>, mut st: std::sync::MutexGuard<'_, super::State>) {
        st.tool_handoff = None;
        st.cancel_pending = false;
        st.cancel.store(true, Ordering::Relaxed);
        st.generation += 1;
        st.busy = false;
        self.set_activity(&mut st, crate::Activity::Idle);
        st.queue.clear();
        let cache = self.cached(&mut st);
        for (i, mut r) in wire::missing_results(&cache.records)
            .into_iter()
            .enumerate()
        {
            if let Record::Tool { id, output, .. } = &mut r {
                if i == 0 {
                    *output = "[cancelled]".into();
                }
                let event = Event::ToolEnd {
                    id: id.clone(),
                    output: output.clone(),
                    code: None,
                    diffs: vec![],
                };
                self.record(&mut st, &[r], [event]);
            }
        }
        self.end_tool_step(&mut st);
        let text = std::mem::take(&mut st.partial);
        let reasoning = std::mem::take(&mut st.reasoning);
        let streaming = std::mem::take(&mut st.streaming);
        if streaming {
            let r = Record::Assistant {
                text: text.clone(),
                reasoning,
                tool_calls: vec![],
                replay: None,
                cancelled: true,
            };
            self.record(&mut st, &[r], [Event::Assistant { text }]);
        }
        let elapsed = st.turn_start.elapsed();
        let cache = self.cached(&mut st);
        let (model, effort) = (cache.meta.model.clone(), cache.meta.effort.clone());
        let calls = st.calls;
        self.record(
            &mut st,
            &[Record::TurnEnd {
                elapsed_ms: elapsed.as_millis() as u64,
                calls,
                model: model.clone(),
                effort: effort.clone(),
                cancelled: true,
            }],
            [Event::TurnEnd {
                elapsed,
                calls,
                model,
                effort,
                cancelled: true,
            }],
        );
        drop(st);
        if streaming {
            self.agent_message_hook("cancelled");
        }
        self.simple_hook("turn_stopped");
    }
}
