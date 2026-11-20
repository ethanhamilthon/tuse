//! A whole `tuse json` session against a fake provider.
#[path = "../../core/tests/common/mod.rs"]
mod common;
use common::Reply;
use serde_json::{Value, json};
use std::io::{BufReader, Write};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex, mpsc};
use std::time::Duration;
use tuse_core::{Mode, Options, Session, Start};

/// Collects what the server writes, line by line.
struct Lines(mpsc::Sender<Value>, Vec<u8>);

impl Write for Lines {
    fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
        self.1.extend_from_slice(b);
        while let Some(n) = self.1.iter().position(|&c| c == b'\n') {
            let line: Vec<u8> = self.1.drain(..=n).collect();
            let v = serde_json::from_slice(&line).expect("stdout carries only JSON lines");
            let _ = self.0.send(v);
        }
        Ok(b.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// The reader side of a pipe the test writes requests into.
struct Pipe(Arc<Mutex<mpsc::Receiver<String>>>, Vec<u8>);

impl std::io::Read for Pipe {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.1.is_empty() {
            match self.0.lock().unwrap().recv() {
                Ok(s) => self.1 = s.into_bytes(),
                Err(_) => return Ok(0),
            }
        }
        let n = buf.len().min(self.1.len());
        buf[..n].copy_from_slice(&self.1[..n]);
        self.1.drain(..n);
        Ok(n)
    }
}

/// Waits for the message matching `f`, skipping the others.
fn until(rx: &mpsc::Receiver<Value>, f: impl Fn(&Value) -> bool) -> Value {
    loop {
        let v = rx.recv_timeout(Duration::from_secs(30)).expect("timed out");
        if f(&v) {
            return v;
        }
    }
}

fn event(rx: &mpsc::Receiver<Value>, ty: &str) -> Value {
    until(rx, |v| v["method"] == "event" && v["params"]["type"] == ty)["params"].clone()
}

fn reply(rx: &mpsc::Receiver<Value>, id: u64) -> Value {
    until(rx, |v| v["id"] == id)
}

#[test]
fn a_client_drives_a_session() {
    let code = "print('answer:', ask('Deploy?', ['yes', 'no']))";
    let addr = common::provider(vec![
        Reply::Call(
            "python",
            json!({"code": code, "title": "Asking"}).to_string(),
        ),
        Reply::Text("deployed"),
    ]);
    let (dir, cfg) = common::setup("rpc", &addr, "");
    let session = Session::open(Options {
        cwd: dir.clone(),
        config: Some(cfg),
        mode: Mode::Json,
        resume: None,
        start: Start::Launch,
    })
    .unwrap();
    let (req_tx, req_rx) = mpsc::channel::<String>();
    let (out_tx, out) = mpsc::channel();
    let input = BufReader::new(Pipe(Arc::new(Mutex::new(req_rx)), vec![]));
    let server = std::thread::spawn(move || {
        tuse_rpc::serve(
            session,
            input,
            tuse_rpc::Out::new(Lines(out_tx, vec![])),
            Arc::new(AtomicBool::new(false)),
        )
    });
    let send = |v: Value| req_tx.send(format!("{v}\n")).unwrap();
    let call = |id: u64, method: &str, params: Value| {
        send(json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}))
    };

    call(1, "session.info", json!({}));
    assert_eq!(reply(&out, 1)["error"]["code"], -32002);
    req_tx.send("{not json\n".into()).unwrap();
    assert_eq!(
        until(&out, |v| v.get("error").is_some())["error"]["code"],
        -32700
    );
    call(
        2,
        "initialize",
        json!({"protocolVersion": 1, "client": {"name": "t"}}),
    );
    let init = reply(&out, 2)["result"].clone();
    assert_eq!(init["protocolVersion"], 1);
    assert_eq!(init["model"], "p/m");
    assert!(init["capabilities"]["methods"].as_array().unwrap().len() > 10);
    call(3, "nope", json!({}));
    assert_eq!(reply(&out, 3)["error"]["code"], -32601);

    call(4, "turn.submit", json!({"text": "deploy it"}));
    assert_eq!(reply(&out, 4)["result"]["queued"], false);
    assert_eq!(event(&out, "user")["text"], "deploy it");
    let ask = event(&out, "ask");
    assert_eq!(ask["question"], "Deploy?");
    call(5, "ask.answer", json!({"id": ask["id"], "answer": "yes"}));
    assert_eq!(reply(&out, 5)["result"], Value::Null);
    let end = event(&out, "tool_end");
    assert!(
        end["output"].as_str().unwrap().contains("answer: yes"),
        "{end}"
    );
    assert_eq!(event(&out, "assistant")["text"], "deployed");
    assert_eq!(event(&out, "turn_end")["cancelled"], false);

    call(6, "session.stats", json!({}));
    let stats = reply(&out, 6)["result"].clone();
    assert_eq!(
        (stats["requests"].clone(), stats["turns"].clone()),
        (json!(2), json!(1))
    );
    call(7, "session.records", json!({}));
    let records = reply(&out, 7)["result"].clone();
    assert!(records.as_array().unwrap().iter().any(|r| r["t"] == "tool"));
    call(8, "ask.answer", json!({"id": "a9", "answer": "x"}));
    assert_eq!(reply(&out, 8)["error"]["code"], -32000);
    call(9, "shutdown", json!({}));
    assert_eq!(reply(&out, 9)["result"], Value::Null);
    server.join().unwrap();
    common::cleanup(&dir);
}

#[test]
fn startup_errors_arrive_after_the_initialize_answer() {
    let addr = common::provider(vec![]);
    let extra =
        "hooks:\n  broken:\n    events: [process_start]\n    run: raise Exception('boom')\n";
    let (dir, cfg) = common::setup("rpc-early", &addr, extra);
    let session = Session::open(Options {
        cwd: dir.clone(),
        config: Some(cfg),
        mode: Mode::Json,
        resume: None,
        start: Start::Launch,
    })
    .unwrap();
    let (req_tx, req_rx) = mpsc::channel::<String>();
    let (out_tx, out) = mpsc::channel();
    let input = BufReader::new(Pipe(Arc::new(Mutex::new(req_rx)), vec![]));
    let server = std::thread::spawn(move || {
        tuse_rpc::serve(
            session,
            input,
            tuse_rpc::Out::new(Lines(out_tx, vec![])),
            Arc::new(AtomicBool::new(false)),
        )
    });
    std::thread::sleep(Duration::from_millis(300));
    let send = |v: Value| req_tx.send(format!("{v}\n")).unwrap();
    send(json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}));
    let first = out.recv_timeout(Duration::from_secs(30)).unwrap();
    assert_eq!(first["id"], 1, "{first}");
    let notice = event(&out, "notice");
    assert!(
        notice["text"].as_str().unwrap().contains("boom"),
        "{notice}"
    );
    send(json!({"jsonrpc": "2.0", "id": 2, "method": "shutdown", "params": {}}));
    reply(&out, 2);
    server.join().unwrap();
    common::cleanup(&dir);
}
