//! `model.list` filtering by mode and remote cache reset on reload.
#[path = "../../core/tests/common/mod.rs"]
mod common;
mod support;

use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex, mpsc};
use support::{Lines, Pipe, event, reply};
use tuse_core::{Mode, Options, Session, Start};

fn mock_models() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap().to_string();
    std::thread::spawn(move || {
        for mut s in listener.incoming().flatten() {
            let mut r = BufReader::new(s.try_clone().unwrap());
            let mut line = String::new();
            while r.read_line(&mut line).unwrap_or(0) > 0 && !line.trim().is_empty() {
                line.clear();
            }
            let body = r#"{"data":[{"id":"remote"}]}"#;
            let _ = write!(
                s,
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = s.flush();
        }
    });
    addr
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

fn rpc_call(
    tx: &mpsc::Sender<String>,
    out: &mpsc::Receiver<Value>,
    id: u64,
    method: &str,
) -> Value {
    let msg = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": {}});
    tx.send(format!("{msg}\n")).unwrap();
    reply(out, id)["result"].clone()
}

#[test]
fn model_list_filters_by_mode_and_drops_stale_remote_on_reload() {
    let addr = mock_models();
    let (dir, _) = common::setup("rpc-models", &addr, "");
    let cfg = dir.join("main.yaml");
    let config_yaml = |json_p_modes: &str| {
        format!(
            r#"connect:
  - {{name: tui_p, base_url: 'http://{addr}', api_key: k, modes: [tui]}}
  - {{name: json_p, base_url: 'http://{addr}', api_key: k, modes: [{json_p_modes}]}}
  - {{name: all_p, base_url: 'http://{addr}', api_key: k}}
models:
  - {{name: tui_p/cfg}}
  - {{name: json_p/cfg}}
  - {{name: all_p/cfg}}
"#
        )
    };
    std::fs::write(&cfg, config_yaml("json")).unwrap();

    // Json mode sees json_p and all_p only (never tui_p).
    let (tx, out, server) = spawn_rpc(&dir, &cfg, Mode::Json);
    rpc_call(&tx, &out, 1, "initialize");
    let initial = rpc_call(&tx, &out, 2, "model.list");
    assert_eq!(initial, json!(["all_p/cfg", "json_p/cfg"]));
    event(&out, "models");
    event(&out, "models");
    let fetched = rpc_call(&tx, &out, 3, "model.list");
    assert_eq!(
        fetched,
        json!(["all_p/cfg", "all_p/remote", "json_p/cfg", "json_p/remote"])
    );

    // After reload excludes json_p, cached remote models are cleared and json_p disappears.
    std::fs::write(&cfg, config_yaml("tui")).unwrap();
    rpc_call(&tx, &out, 4, "config.reload");
    let after_reload = rpc_call(&tx, &out, 5, "model.list");
    assert_eq!(after_reload, json!(["all_p/cfg"]));
    event(&out, "models");
    let after_refetch = rpc_call(&tx, &out, 6, "model.list");
    assert_eq!(after_refetch, json!(["all_p/cfg", "all_p/remote"]));
    rpc_call(&tx, &out, 7, "shutdown");
    server.join().unwrap();

    // Tui mode on the original config sees tui_p and all_p only (never json_p).
    std::fs::write(&cfg, config_yaml("json")).unwrap();
    let (tx, out, server) = spawn_rpc(&dir, &cfg, Mode::Tui);
    rpc_call(&tx, &out, 1, "initialize");
    let initial = rpc_call(&tx, &out, 2, "model.list");
    assert_eq!(initial, json!(["all_p/cfg", "tui_p/cfg"]));
    event(&out, "models");
    event(&out, "models");
    let fetched = rpc_call(&tx, &out, 3, "model.list");
    assert_eq!(
        fetched,
        json!(["all_p/cfg", "all_p/remote", "tui_p/cfg", "tui_p/remote"])
    );
    rpc_call(&tx, &out, 4, "shutdown");
    server.join().unwrap();

    common::cleanup(&dir);
}
