#[path = "../../core/tests/common/mod.rs"]
mod common;
mod support;

use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::path::Path;
use std::sync::mpsc;
use std::time::Duration;
use tuse_core::{Mode, Options, Session, Start};

type GateReq = (mpsc::Sender<()>, mpsc::Receiver<()>);

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
    let session = Session::open({
        unsafe {
            std::env::set_var("TUSE_GLOBAL_CONFIG", cfg);
        }
        Options {
            cwd: dir.to_path_buf(),
            mode,
            resume: None,
            start: Start::Launch,
            flags: Default::default(),
        }
    })
    .unwrap();
    support::spawn(session)
}

fn providers(list: &[(&str, String)]) {
    for p in tuse_core::Providers::list().unwrap() {
        tuse_core::Providers::remove(&p.name).unwrap();
    }
    for (name, url) in list {
        tuse_core::Providers::add(&tuse_config::Connect {
            name: name.to_string(),
            base_url: url.clone(),
            api_key: "k".into(),
            ..Default::default()
        })
        .unwrap();
    }
}

fn rpc_call(
    tx: &mpsc::Sender<String>,
    out: &mpsc::Receiver<Value>,
    id: u64,
    method: &str,
) -> Value {
    let msg = json!({"jsonrpc": "2.0", "id": id, "method": method});
    tx.send(format!("{msg}\n")).unwrap();
    support::until(out, |v| {
        assert_ne!(v["params"]["type"], "error", "unexpected error event: {v}");
        v["id"] == id
    })["result"]
        .clone()
}

fn wait_models_event(out: &mpsc::Receiver<Value>) {
    support::until(out, |v| {
        assert_ne!(v["params"]["type"], "error", "unexpected error event: {v}");
        v["method"] == "event" && v["params"]["type"] == "models"
    });
}

#[test]
fn model_list_discards_stale_fetches_on_reload() {
    let (old_addr, old_req) = gated_models(Some("old_only"));
    let (new_addr, new_req) = gated_models(Some("new_only"));
    let (fresh_addr, fresh_req) = gated_models(Some("fresh"));
    let (err_addr, err_req) = gated_models(None);
    let (dir, _) = common::setup("rpc-models", &old_addr, "");
    let cfg = dir.join("main.lua");

    providers(&[
        ("p", format!("http://{old_addr}")),
        ("broken", format!("http://{err_addr}")),
    ]);
    std::fs::write(&cfg, "return { models = { ['p/cfg'] = {} } }").unwrap();
    let (tx, out, server) = spawn_rpc(&dir, &cfg, Mode::Json);
    rpc_call(&tx, &out, 1, "initialize");
    assert_eq!(rpc_call(&tx, &out, 2, "model.list"), json!([]));
    let (rel_old, done_old) = old_req.recv_timeout(Duration::from_secs(5)).unwrap();
    let (rel_err, done_err) = err_req.recv_timeout(Duration::from_secs(5)).unwrap();

    assert_eq!(rpc_call(&tx, &out, 20, "model.list"), json!([]));
    assert!(old_req.recv_timeout(Duration::from_millis(50)).is_err());
    assert!(err_req.try_recv().is_err());
    providers(&[("p", format!("http://{new_addr}"))]);
    rpc_call(&tx, &out, 3, "config.reload");
    support::event(&out, "reloaded");
    assert_eq!(rpc_call(&tx, &out, 4, "model.list"), json!([]));
    let (rel_new, _) = new_req.recv_timeout(Duration::from_secs(5)).unwrap();
    rel_new.send(()).unwrap();
    wait_models_event(&out);
    rel_old.send(()).unwrap();
    rel_err.send(()).unwrap();
    done_old.recv_timeout(Duration::from_secs(5)).unwrap();
    done_err.recv_timeout(Duration::from_secs(5)).unwrap();
    assert_eq!(rpc_call(&tx, &out, 5, "model.list"), json!(["p/new_only"]));

    providers(&[("all_p", format!("http://{fresh_addr}"))]);
    std::fs::write(&cfg, "return { models = { ['all_p/cfg'] = {} } }").unwrap();
    rpc_call(&tx, &out, 6, "config.reload");
    assert_eq!(rpc_call(&tx, &out, 7, "model.list"), json!([]));
    let (rel_fresh, _) = fresh_req.recv_timeout(Duration::from_secs(5)).unwrap();
    rel_fresh.send(()).unwrap();
    wait_models_event(&out);
    assert_eq!(rpc_call(&tx, &out, 8, "model.list"), json!(["all_p/fresh"]));
    support::call(&tx, &out, 9, "shutdown", json!({}));
    server.join().unwrap();

    common::cleanup(&dir);
}
