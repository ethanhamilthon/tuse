use serde_json::Value;
use std::io::Write;
use std::sync::{Arc, Mutex, mpsc};
use std::time::Duration;

pub struct Lines(pub mpsc::Sender<Value>, pub Vec<u8>);

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

pub struct Pipe(pub Arc<Mutex<mpsc::Receiver<String>>>, pub Vec<u8>);

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

pub fn until(rx: &mpsc::Receiver<Value>, f: impl Fn(&Value) -> bool) -> Value {
    loop {
        let v = rx.recv_timeout(Duration::from_secs(30)).expect("timed out");
        if f(&v) {
            return v;
        }
    }
}

pub fn event(rx: &mpsc::Receiver<Value>, ty: &str) -> Value {
    until(rx, |v| v["method"] == "event" && v["params"]["type"] == ty)["params"].clone()
}

pub fn spawn(
    session: Arc<tuse_core::Session>,
) -> (
    mpsc::Sender<String>,
    mpsc::Receiver<Value>,
    std::thread::JoinHandle<()>,
) {
    let (req_tx, req_rx) = mpsc::channel::<String>();
    let (out_tx, out) = mpsc::channel();
    let input = std::io::BufReader::new(Pipe(Arc::new(Mutex::new(req_rx)), vec![]));
    let server = std::thread::spawn(move || {
        tuse_rpc::serve(
            session,
            input,
            tuse_rpc::Out::new(Lines(out_tx, vec![])),
            Arc::new(std::sync::atomic::AtomicBool::new(false)),
        )
    });
    (req_tx, out, server)
}

pub fn call(
    tx: &mpsc::Sender<String>,
    out: &mpsc::Receiver<Value>,
    id: u64,
    method: &str,
    params: Value,
) -> Value {
    let msg = serde_json::json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
    tx.send(format!("{msg}\n")).unwrap();
    until(out, |v| v["id"] == id)
}
