//! `tuse json`: a JSON-RPC 2.0 session over stdin and stdout, one message per line.
mod events;
mod methods;
mod proto;

use proto::{Fail, NOT_READY, answer, notify};
use serde_json::Value;
use std::io::{BufRead, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tuse_core::Session;

/// One writer for answers and notifications, so lines never interleave.
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

/// Serves `session` until the input ends, `shutdown` arrives or `stop` is set; then
/// closes the session (tasks stop, `process_end` hooks run).
pub fn serve(
    session: Arc<Session>,
    input: impl BufRead + Send + 'static,
    out: Out,
    stop: Arc<AtomicBool>,
) {
    // Set only after the `initialize` answer is written; until then events wait in the channel.
    let ready = Arc::new(AtomicBool::new(false));
    let events = session.subscribe();
    let done = Arc::new(AtomicBool::new(false));
    let pump = {
        let (out, ready, done) = (out.clone(), ready.clone(), done.clone());
        std::thread::spawn(move || {
            while !done.load(Ordering::Relaxed) {
                if !ready.load(Ordering::Acquire) {
                    std::thread::sleep(Duration::from_millis(10));
                } else if let Ok(e) = events.recv_timeout(Duration::from_millis(50)) {
                    out.send(&notify("event", events::to_json(&e)));
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
    loop {
        if stop.load(Ordering::Relaxed) {
            break;
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
                let reply = Ok(methods::initialize(&session));
                if let Some(id) = req.id {
                    out.send(&answer(id, reply));
                }
                ready.store(true, Ordering::Release);
                continue;
            }
            "shutdown" => {
                if let Some(id) = req.id {
                    out.send(&answer(id, Ok(Value::Null)));
                }
                break;
            }
            _ if !ready.load(Ordering::Acquire) => {
                Err(Fail(NOT_READY, "call `initialize` first".into()))
            }
            m => methods::call(&session, m, &req.params),
        };
        if let Some(id) = req.id {
            out.send(&answer(id, reply));
        }
    }
    session.close();
    // Events of the closing (`process_end` notices and the like) still reach the client.
    std::thread::sleep(Duration::from_millis(100));
    done.store(true, Ordering::Relaxed);
    let _ = pump.join();
}
