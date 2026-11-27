mod events;
mod methods;
mod proto;

pub use methods::METHODS;
use proto::{Fail, NOT_READY, answer, notify};
use serde_json::Value;
use std::io::{BufRead, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tuse_core::Session;

#[derive(Clone)]
pub struct Out(Arc<Mutex<Box<dyn Write + Send>>>);

impl Out {
    pub fn new(w: impl Write + Send + 'static) -> Self {
        Out(Arc::new(Mutex::new(Box::new(w))))
    }

    fn send(&self, v: &Value) {
        let mut w = self.0.lock().unwrap();
        let _ = writeln!(w, "{v}");
        let _ = w.flush();
    }
}

pub fn serve(
    mut session: Arc<Session>,
    input: impl BufRead + Send + 'static,
    out: Out,
    stop: Arc<AtomicBool>,
) {
    let ready = Arc::new(AtomicBool::new(false));
    let mut events = session.subscribe();
    let done = Arc::new(AtomicBool::new(false));
    let (swap_tx, swap_rx) = std::sync::mpsc::channel::<(
        std::sync::mpsc::Receiver<tuse_core::Event>,
        std::sync::mpsc::Sender<()>,
    )>();
    let (switch_tx, switch_rx) = std::sync::mpsc::channel::<tuse_core::Switch>();
    let pump = {
        let (out, ready, done) = (out.clone(), ready.clone(), done.clone());
        std::thread::spawn(move || {
            let forward = |e: tuse_core::Event| match e {
                tuse_core::Event::Switch(switch) => {
                    let _ = switch_tx.send(switch);
                }
                e => {
                    if let Some(v) = events::to_json(&e) {
                        out.send(&notify("event", v));
                    }
                }
            };
            while !done.load(Ordering::Relaxed) {
                if let Ok((next, ack)) = swap_rx.try_recv() {
                    while let Ok(e) = events.try_recv() {
                        forward(e);
                    }
                    events = next;
                    let _ = ack.send(());
                    continue;
                }
                if !ready.load(Ordering::Acquire) {
                    std::thread::sleep(Duration::from_millis(10));
                } else if let Ok(e) = events.recv_timeout(Duration::from_millis(20)) {
                    forward(e);
                }
            }
        })
    };
    let (lines_tx, lines) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        for line in input.lines() {
            if lines_tx.send(line).is_err() {
                break;
            }
        }
    });
    let mut shutdown = false;
    let swap = |session: &mut Arc<Session>, next: Arc<Session>| {
        ready.store(false, Ordering::Release);
        let (ack_tx, ack_rx) = std::sync::mpsc::channel();
        let _ = swap_tx.send((next.subscribe(), ack_tx));
        let _ = ack_rx.recv();
        *session = next;
    };
    loop {
        if stop.load(Ordering::Relaxed) {
            break;
        }
        if let Ok(switch) = switch_rx.try_recv() {
            let switched = match &switch {
                tuse_core::Switch::Fork(index) => session.fork(*index),
                tuse_core::Switch::Resume(id) => session
                    .switch(Some(id.clone()))
                    .map(|next| (next, String::new())),
            };
            match switched {
                Ok((next, draft)) => {
                    swap(&mut session, next);
                    let mut event = methods::switched(&session);
                    event["type"] = "session_switched".into();
                    event["draft"] = draft.into();
                    out.send(&notify("event", event));
                    ready.store(true, Ordering::Release);
                }
                Err(e) => session.error(e),
            }
        }
        let line = match lines.recv_timeout(Duration::from_millis(100)) {
            Ok(Ok(line)) => line,
            Ok(Err(_)) | Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
            Err(_) => continue,
        };
        if line.trim().is_empty() {
            continue;
        }
        let req = match proto::parse(&line) {
            Ok(r) => r,
            Err((id, fail)) => {
                out.send(&answer(id, Err(fail)));
                continue;
            }
        };
        let reply = match req.method.as_str() {
            "initialize" => {
                let reply = methods::initialize(&session, &req.params);
                let ok = reply.is_ok();
                if let Some(id) = req.id {
                    out.send(&answer(id, reply));
                }
                if ok {
                    ready.store(true, Ordering::Release);
                }
                continue;
            }
            "shutdown" => {
                shutdown = true;
                if let Some(id) = req.id {
                    out.send(&answer(id, Ok(Value::Null)));
                }
                break;
            }
            _ if !ready.load(Ordering::Acquire) => {
                Err(Fail(NOT_READY, "call `initialize` first".into()))
            }
            "session.new" | "session.fork" => {
                let switched = match req.method.as_str() {
                    "session.fork" => proto::index_param(&req.params, "index").and_then(|index| {
                        let (next, draft) = session.fork(index).map_err(Fail::session)?;
                        Ok((next, Some(draft)))
                    }),
                    _ => session
                        .switch(None)
                        .map(|next| (next, None))
                        .map_err(Fail::session),
                };
                match switched {
                    Ok((next, draft)) => {
                        swap(&mut session, next);
                        if let Some(id) = req.id {
                            let mut result = methods::switched(&session);
                            if let Some(draft) = draft {
                                result["draft"] = draft.into();
                            }
                            out.send(&answer(id, Ok(result)));
                        }
                        ready.store(true, Ordering::Release);
                    }
                    Err(f) => {
                        if let Some(id) = req.id {
                            out.send(&answer(id, Err(f)));
                        }
                    }
                }
                continue;
            }
            m => methods::call(&session, m, &req.params),
        };
        if let Some(id) = req.id {
            out.send(&answer(id, reply));
        }
    }
    if shutdown {
        session.close();
    } else {
        session.disconnect();
    }
    std::thread::sleep(Duration::from_millis(100));
    done.store(true, Ordering::Relaxed);
    let _ = pump.join();
}
