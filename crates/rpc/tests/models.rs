//! `model.list` filtering by mode, reload cache reset, and stale in-flight fetch discard.
#[path = "../../core/tests/common/mod.rs"]
mod common;
#[allow(dead_code)]
mod support;

use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex, mpsc};
use std::time::Duration;
use support::{Lines, Pipe};
use tuse_core::{Mode, Options, Session, Start};

type GateReq = (mpsc::Sender<()>, mpsc::Receiver<()>);

/// A mock `/models` server that blocks each request until its release sender fires,
/// then signals completion once the client finishes reading and closes the socket.
/// `None` closes the socket without a response to trigger a fetch error.
fn gated_models(model_id: Option<&'static str>) -> (String, mpsc::Receiver<GateReq>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap().to_string();
    let (arr_tx, arrived) = mpsc::channel::<GateReq>();
    std::thread::spawn(move || {
        for mut s in listener.incoming().flatten() {
            let arr_tx = arr_tx.clone();
            std::thread::spawn(move || {
                use std::io::Read;
                let mut r = BufReader::new(s.try_clone().unwrap());
                let mut line = String::new();
                while r.read_line(&mut line).unwrap_or(0) > 0 && !line.trim().is_empty() {
                    line.clear();
                }
                let (rel_tx, rel_rx) = mpsc::channel();
                let (done_tx, done_rx) = mpsc::channel();
                if arr_tx.send((rel_tx, done_rx)).is_err() || rel_rx.recv().is_err() {
                    return;
                }
                if let Some(id) = model_id {
                    let body = format!(r#"{{"data":[{{"id":"{id}"}}]}}"#);
                    let _ = write!(
                        s,
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                        body.len()
                    );
                    let _ = s.flush();
                    let _ = s.shutdown(std::net::Shutdown::Write);
                    let _ = s.read(&mut [0u8; 1]);
                } else {
                    drop(s);
                }
                let _ = done_tx.send(());
            });
        }
    });
    (addr, arrived)
}

fn spawn_rpc(
    dir: &Path,
    cfg: &Path,
    mode: Mode,
) -> (
    mpsc::Sender<String>,
    mpsc::Receiver<Value>,
    std::thread::JoinHandle<()>,
) {
    let session = Session::open(Options {
        cwd: dir.to_path_buf(),
        config: Some(cfg.to_path_buf()),
        mode,
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
    (req_tx, out, server)
}

fn next_until(out: &mpsc::Receiver<Value>, f: impl Fn(&Value) -> bool) -> Value {
    loop {
        let v = out
            .recv_timeout(Duration::from_secs(10))
            .expect("timed out");
        if v["method"] == "event" && v["params"]["type"] == "error" {
            panic!("unexpected error event: {v}");
        }
        if f(&v) {
            return v;
        }
    }
}

fn rpc_call(
    tx: &mpsc::Sender<String>,
    out: &mpsc::Receiver<Value>,
    id: u64,
    method: &str,
) -> Value {
    let msg = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": {}});
    tx.send(format!("{msg}\n")).unwrap();
    next_until(out, |v| v["id"] == id)["result"].clone()
}

fn wait_models_event(out: &mpsc::Receiver<Value>) {
    next_until(out, |v| {
        v["method"] == "event" && v["params"]["type"] == "models"
    });
}

#[test]
fn model_list_filters_by_mode_and_discards_stale_fetches_on_reload() {
    let (old_addr, old_req) = gated_models(Some("old_only"));
    let (new_addr, new_req) = gated_models(Some("new_only"));
    let (fresh_addr, fresh_req) = gated_models(Some("fresh"));
    let (err_addr, err_req) = gated_models(None);
    let (tui_addr, tui_req) = gated_models(Some("tui_remote"));
    let (dir, _) = common::setup("rpc-models", &old_addr, "");
    let cfg = dir.join("main.yaml");

    // 1. Json mode: hold in-flight fetches for old `p` and `broken`, then reload before releasing.
    std::fs::write(
        &cfg,
        format!(
            r#"connect:
  - {{name: tui_p, base_url: 'http://{tui_addr}', api_key: k, modes: [tui]}}
  - {{name: p, base_url: 'http://{old_addr}', api_key: k}}
  - {{name: broken, base_url: 'http://{err_addr}', api_key: k}}
models:
  - {{name: tui_p/cfg}}
  - {{name: p/cfg}}
"#
        ),
    )
    .unwrap();
    let (tx, out, server) = spawn_rpc(&dir, &cfg, Mode::Json);
    rpc_call(&tx, &out, 1, "initialize");
    assert_eq!(rpc_call(&tx, &out, 2, "model.list"), json!(["p/cfg"]));
    let (rel_old, done_old) = old_req.recv_timeout(Duration::from_secs(5)).unwrap();
    let (rel_err, done_err) = err_req.recv_timeout(Duration::from_secs(5)).unwrap();
    assert!(tui_req.try_recv().is_err(), "tui_p queried in json mode");

    // Reload: old `p` and `broken` become tui-only; a new json `p` takes over the same name.
    std::fs::write(
        &cfg,
        format!(
            r#"connect:
  - {{name: p, base_url: 'http://{old_addr}', api_key: k, modes: [tui]}}
  - {{name: p, base_url: 'http://{new_addr}', api_key: k, modes: [json]}}
  - {{name: broken, base_url: 'http://{err_addr}', api_key: k, modes: [tui]}}
models:
  - {{name: p/cfg}}
"#
        ),
    )
    .unwrap();
    rpc_call(&tx, &out, 3, "config.reload");
    assert_eq!(rpc_call(&tx, &out, 4, "model.list"), json!(["p/cfg"]));
    let (rel_new, _) = new_req.recv_timeout(Duration::from_secs(5)).unwrap();
    rel_new.send(()).unwrap();
    wait_models_event(&out);
    // Release stale response and stale error AFTER new_only is cached: both must be discarded.
    rel_old.send(()).unwrap();
    rel_err.send(()).unwrap();
    done_old.recv_timeout(Duration::from_secs(5)).unwrap();
    done_err.recv_timeout(Duration::from_secs(5)).unwrap();
    assert_eq!(
        rpc_call(&tx, &out, 5, "model.list"),
        json!(["p/cfg", "p/new_only"])
    );

    // Reload drops cached `p/new_only` when `p` leaves json mode; fresh fetch is gated.
    std::fs::write(
        &cfg,
        format!(
            r#"connect:
  - {{name: p, base_url: 'http://{new_addr}', api_key: k, modes: [tui]}}
  - {{name: all_p, base_url: 'http://{fresh_addr}', api_key: k}}
models:
  - {{name: p/cfg}}
  - {{name: all_p/cfg}}
"#
        ),
    )
    .unwrap();
    rpc_call(&tx, &out, 6, "config.reload");
    assert_eq!(rpc_call(&tx, &out, 7, "model.list"), json!(["all_p/cfg"]));
    let (rel_fresh, _) = fresh_req.recv_timeout(Duration::from_secs(5)).unwrap();
    rel_fresh.send(()).unwrap();
    wait_models_event(&out);
    assert_eq!(
        rpc_call(&tx, &out, 8, "model.list"),
        json!(["all_p/cfg", "all_p/fresh"])
    );
    rpc_call(&tx, &out, 9, "shutdown");
    server.join().unwrap();

    // 2. Tui mode sees tui_p only (never json_p).
    let (json_only_addr, json_only_req) = gated_models(Some("json_remote"));
    std::fs::write(
        &cfg,
        format!(
            r#"connect:
  - {{name: tui_p, base_url: 'http://{tui_addr}', api_key: k, modes: [tui]}}
  - {{name: json_p, base_url: 'http://{json_only_addr}', api_key: k, modes: [json]}}
models:
  - {{name: tui_p/cfg}}
  - {{name: json_p/cfg}}
"#
        ),
    )
    .unwrap();
    let (tx, out, server) = spawn_rpc(&dir, &cfg, Mode::Tui);
    rpc_call(&tx, &out, 1, "initialize");
    assert_eq!(rpc_call(&tx, &out, 2, "model.list"), json!(["tui_p/cfg"]));
    let (rel_tui, _) = tui_req.recv_timeout(Duration::from_secs(5)).unwrap();
    assert!(json_only_req.try_recv().is_err(), "json_p queried in tui");
    rel_tui.send(()).unwrap();
    wait_models_event(&out);
    assert_eq!(
        rpc_call(&tx, &out, 3, "model.list"),
        json!(["tui_p/cfg", "tui_p/tui_remote"])
    );
    rpc_call(&tx, &out, 4, "shutdown");
    server.join().unwrap();

    common::cleanup(&dir);
}
