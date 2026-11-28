use flate2::Compression;
use flate2::write::GzEncoder;
use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::path::Path;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Barrier};

fn tgz(files: &[(&str, &str)]) -> Vec<u8> {
    let mut enc = GzEncoder::new(Vec::new(), Compression::default());
    {
        let mut tar = tar::Builder::new(&mut enc);
        for (path, body) in files {
            let mut h = tar::Header::new_gnu();
            h.set_size(body.len() as u64);
            h.set_mode(0o644);
            h.set_cksum();
            tar.append_data(&mut h, path, body.as_bytes()).unwrap();
        }
        tar.finish().unwrap();
    }
    enc.finish().unwrap()
}

fn serve(routes: Vec<(String, Vec<u8>)>) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let routes = Arc::new(routes);
    std::thread::spawn(move || {
        for mut stream in listener.incoming().flatten() {
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut req = String::new();
            let _ = reader.read_line(&mut req);
            let path = req.split_whitespace().nth(1).unwrap_or("");
            let mut line = String::new();
            while reader.read_line(&mut line).unwrap_or(0) > 0 && !line.trim().is_empty() {
                line.clear();
            }
            if let Some((_, body)) = routes.iter().find(|(p, _)| p == path) {
                let hdr = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/gzip\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                let _ = stream.write_all(hdr.as_bytes());
                let _ = stream.write_all(body);
            } else {
                let _ = stream.write_all(
                    b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                );
            }
        }
    });
    url
}

fn stable_archive(ver: &str) -> Vec<u8> {
    let manifest = format!(r#"{{"name":"stable","version":"{ver}"}}"#);
    let init = format!(
        r#"
local extra = require("extra")
assert(extra.ver == "{ver}", "stable init/extra mismatch: {ver} vs " .. tostring(extra.ver))
return {{
  prompts = {{ stable = "{ver}:" .. extra.ver }},
  commands = {{
    stable_check = function()
        local lazy = require("lazy")
        require("tuse").ui.show("stable_check", "{ver}:" .. extra.ver .. ":" .. lazy.ver, false)
    end,
  }},
}}
"#
    );
    let extra = format!(r#"return {{ ver = "{ver}" }}"#);
    let lazy = format!(r#"return {{ ver = "{ver}" }}"#);
    tgz(&[
        ("r/tuse-plugin.json", &manifest),
        ("r/init.lua", &init),
        ("r/extra.lua", &extra),
        ("r/lazy.lua", &lazy),
    ])
}

fn churn_archive(ver: &str) -> Vec<u8> {
    let manifest = format!(r#"{{"name":"churn","version":"{ver}"}}"#);
    let init = format!(
        r#"
local extra = require("extra")
assert(extra.ver == "{ver}", "churn init/extra mismatch: {ver} vs " .. tostring(extra.ver))
return {{
  prompts = {{ churn = "{ver}:" .. extra.ver }},
}}
"#
    );
    let extra = format!(r#"return {{ ver = "{ver}" }}"#);
    tgz(&[
        ("r/tuse-plugin.json", &manifest),
        ("r/init.lua", &init),
        ("r/extra.lua", &extra),
    ])
}

fn selfmut_archive() -> Vec<u8> {
    tgz(&[
        (
            "r/tuse-plugin.json",
            r#"{"name":"selfmut","version":"1.0.0"}"#,
        ),
        (
            "r/init.lua",
            r#"
local t = require("tuse")
local deleted = t.plugins.delete("selfmut")
assert(deleted.name == "selfmut")
local extra = require("extra")
return { prompts = { selfmut = "1.0.0:" .. extra.ver } }
"#,
        ),
        ("r/extra.lua", r#"return { ver = "1.0.0" }"#),
    ])
}

fn fixture_server() -> String {
    serve(vec![
        ("/acme/stable.tar.gz".into(), stable_archive("1.0.0")),
        ("/acme/stable/v2.tar.gz".into(), stable_archive("2.0.0")),
        ("/acme/churn.tar.gz".into(), churn_archive("1.0.0")),
        ("/acme/churn/v2.tar.gz".into(), churn_archive("2.0.0")),
        ("/acme/selfmut.tar.gz".into(), selfmut_archive()),
    ])
}

struct RpcProc {
    child: Child,
    stdin: Option<ChildStdin>,
    stdout: BufReader<ChildStdout>,
    next_id: u64,
    events: Vec<Value>,
}

impl RpcProc {
    fn spawn(
        cwd: &Path,
        fake_home: &Path,
        tuse_home: &Path,
        url: &str,
        extra_args: &[&str],
    ) -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_tuse"))
            .current_dir(cwd)
            .env("HOME", fake_home)
            .env("TUSE_HOME", tuse_home)
            .env("TUSE_PLUGINS_URL", url)
            .env("TUSE_GLOBAL_CONFIG", cwd.join("tuse.lua"))
            .arg("json")
            .args(extra_args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let stdin = child.stdin.take().unwrap();
        let stdout = BufReader::new(child.stdout.take().unwrap());
        let mut proc = Self {
            child,
            stdin: Some(stdin),
            stdout,
            next_id: 1,
            events: vec![],
        };
        let init = proc.call("initialize", json!({}));
        assert!(init["error"].is_null(), "initialize failed: {init}");
        proc
    }

    fn stderr_text(&mut self) -> String {
        let mut buf = String::new();
        if let Some(mut err) = self.child.stderr.take() {
            let _ = err.read_to_string(&mut buf);
        }
        buf
    }

    fn call(&mut self, method: &str, params: Value) -> Value {
        let id = self.next_id;
        self.next_id += 1;
        let msg = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        });
        let stdin = self.stdin.as_mut().unwrap();
        writeln!(stdin, "{msg}").unwrap();
        stdin.flush().unwrap();
        loop {
            let mut line = String::new();
            let n = self.stdout.read_line(&mut line).unwrap();
            if n == 0 {
                let status = self.child.wait().ok();
                let stderr = self.stderr_text();
                panic!("EOF waiting for {method} (id={id}), status={status:?}, stderr={stderr}");
            }
            let v: Value = serde_json::from_str(&line).unwrap_or_else(|e| {
                panic!("invalid json line `{line}`: {e}");
            });
            if v["id"] == id {
                return v;
            }
            self.events.push(v);
        }
    }

    fn wait_markdown(&mut self, name: &str, prefix: &str) -> String {
        if let Some(pos) = self.events.iter().position(|v| {
            v["method"] == "event"
                && v["params"]["type"] == "ui_markdown"
                && v["params"]["name"] == name
                && v["params"]["text"]
                    .as_str()
                    .is_some_and(|t| t.starts_with(prefix))
        }) {
            return self.events.remove(pos)["params"]["text"]
                .as_str()
                .unwrap()
                .to_string();
        }
        loop {
            let mut line = String::new();
            let n = self.stdout.read_line(&mut line).unwrap();
            if n == 0 {
                let status = self.child.wait().ok();
                let stderr = self.stderr_text();
                panic!(
                    "EOF waiting for ui_markdown {name}/{prefix}, status={status:?}, stderr={stderr}"
                );
            }
            let v: Value = serde_json::from_str(&line).unwrap();
            if v["method"] == "event"
                && v["params"]["type"] == "ui_markdown"
                && v["params"]["name"] == name
                && v["params"]["text"]
                    .as_str()
                    .is_some_and(|t| t.starts_with(prefix))
            {
                return v["params"]["text"].as_str().unwrap().to_string();
            }
            self.events.push(v);
        }
    }

    fn shutdown(mut self) {
        let res = self.call("shutdown", json!({}));
        assert!(res["error"].is_null(), "shutdown failed: {res}");
        drop(self.stdin.take());
        let status = self.child.wait().unwrap();
        assert!(status.success(), "stderr: {}", self.stderr_text());
    }
}

fn plugin_version(cfg: &Value, name: &str) -> Option<String> {
    cfg["plugins"].as_array().and_then(|arr| {
        arr.iter()
            .find(|p| p["name"] == name)
            .and_then(|p| p["version"].as_str())
            .map(str::to_string)
    })
}

fn assert_version_consistent(cfg: &Value, require_stable: bool) -> Option<String> {
    let stable_ver = plugin_version(cfg, "stable");
    if require_stable {
        assert!(
            stable_ver.is_some(),
            "stable plugin vanished from config: {cfg}"
        );
    }
    if let Some(ref ver) = stable_ver {
        assert!(matches!(ver.as_str(), "1.0.0" | "2.0.0"), "{cfg}");
        assert_eq!(
            cfg["prompts"]["stable:stable"].as_str(),
            Some(format!("{ver}:{ver}").as_str()),
            "stable manifest vs prompt mismatch: {cfg}"
        );
    } else {
        assert!(cfg["prompts"]["stable:stable"].is_null(), "{cfg}");
    }
    if let Some(ver) = plugin_version(cfg, "churn") {
        assert!(matches!(ver.as_str(), "1.0.0" | "2.0.0"), "{cfg}");
        assert_eq!(
            cfg["prompts"]["churn:churn"].as_str(),
            Some(format!("{ver}:{ver}").as_str()),
            "churn manifest vs prompt mismatch: {cfg}"
        );
    } else {
        assert!(cfg["prompts"]["churn:churn"].is_null(), "{cfg}");
    }
    stable_ver
}

#[test]
fn first_install_bootstrap_race_when_plugins_dir_and_lock_absent() {
    let url = fixture_server();
    let root = std::env::temp_dir().join(format!("tuse-plug-boot-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let fake_home = root.join("home");
    let proj = root.join("proj");
    std::fs::create_dir_all(&fake_home).unwrap();
    std::fs::create_dir_all(&proj).unwrap();
    std::fs::write(
        proj.join("tuse.lua"),
        "return { work = { prompts = { w = '1' } } }\n",
    )
    .unwrap();

    for round in 0..6 {
        let tuse_home = root.join(format!("tuse-home-{round}"));
        assert!(!tuse_home.exists());
        if round % 2 == 1 {
            std::fs::create_dir_all(&tuse_home).unwrap();
        }
        assert!(!tuse_home.join("plugins").exists());
        assert!(!tuse_home.join(".plugins.lock").exists());

        let barrier = Arc::new(Barrier::new(4));
        let stop = Arc::new(AtomicBool::new(false));
        let observed_installed = Arc::new(AtomicUsize::new(0));

        let mut readers = vec![];
        for _ in 0..3 {
            let (proj, fake_home, tuse_home, url, barrier, stop, observed_installed) = (
                proj.clone(),
                fake_home.clone(),
                tuse_home.clone(),
                url.clone(),
                barrier.clone(),
                stop.clone(),
                observed_installed.clone(),
            );
            readers.push(std::thread::spawn(move || {
                barrier.wait();
                while !stop.load(Ordering::Relaxed) {
                    let mut p = RpcProc::spawn(&proj, &fake_home, &tuse_home, &url, &[]);
                    let cfg = p.call("config.get", json!({}));
                    assert!(cfg["error"].is_null(), "{cfg}");
                    if assert_version_consistent(&cfg["result"], false).is_some() {
                        observed_installed.fetch_add(1, Ordering::Relaxed);
                    }
                    let list = p.call("plugins.list", json!({}));
                    assert!(list["error"].is_null(), "{list}");
                    let rel = p.call("config.reload", json!({}));
                    assert!(rel["error"].is_null(), "{rel}");
                    let cfg2 = p.call("config.get", json!({}));
                    assert!(cfg2["error"].is_null(), "{cfg2}");
                    if assert_version_consistent(&cfg2["result"], false).is_some() {
                        observed_installed.fetch_add(1, Ordering::Relaxed);
                    }
                    p.shutdown();
                }
            }));
        }

        let writer = {
            let (proj, fake_home, tuse_home, url, barrier) = (
                proj.clone(),
                fake_home.clone(),
                tuse_home.clone(),
                url.clone(),
                barrier.clone(),
            );
            std::thread::spawn(move || {
                barrier.wait();
                let mut w = RpcProc::spawn(&proj, &fake_home, &tuse_home, &url, &["--no-plugins"]);
                let add1 = w.call("plugins.add", json!({ "source": "acme/stable" }));
                assert!(add1["error"].is_null(), "{add1}");
                let add2 = w.call(
                    "plugins.add",
                    json!({ "source": "acme/stable@v2", "replace": true }),
                );
                assert!(add2["error"].is_null(), "{add2}");
                w.shutdown();
            })
        };

        writer.join().unwrap();
        stop.store(true, Ordering::Relaxed);
        for r in readers {
            r.join().unwrap();
        }
        let mut verify = RpcProc::spawn(&proj, &fake_home, &tuse_home, &url, &[]);
        let cfg = verify.call("config.get", json!({}));
        assert_eq!(
            assert_version_consistent(&cfg["result"], true).as_deref(),
            Some("2.0.0")
        );
        verify.shutdown();
    }

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn cross_process_startup_and_full_reload_stay_version_consistent_under_mutations() {
    let url = fixture_server();
    let root = std::env::temp_dir().join(format!("tuse-plug-xproc-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let fake_home = root.join("home");
    let tuse_home = root.join("tuse-home");
    let proj = root.join("proj");
    std::fs::create_dir_all(&fake_home).unwrap();
    std::fs::create_dir_all(&proj).unwrap();
    let cfg_path = proj.join("tuse.lua");
    std::fs::write(&cfg_path, "return { work = { prompts = { w = 'v1' } } }\n").unwrap();

    let mut admin = RpcProc::spawn(&proj, &fake_home, &tuse_home, &url, &["--no-plugins"]);
    let added = admin.call("plugins.add", json!({ "source": "acme/stable" }));
    assert!(added["error"].is_null(), "{added}");
    assert_eq!(added["result"]["version"], "1.0.0");

    let stop = Arc::new(AtomicBool::new(false));
    let reloads = Arc::new(AtomicUsize::new(0));
    let startups = Arc::new(AtomicUsize::new(0));

    let reload_reader = {
        let (proj, fake_home, tuse_home, url, stop, reloads) = (
            proj.clone(),
            fake_home.clone(),
            tuse_home.clone(),
            url.clone(),
            stop.clone(),
            reloads.clone(),
        );
        std::thread::spawn(move || {
            let mut reader = RpcProc::spawn(&proj, &fake_home, &tuse_home, &url, &[]);
            let mut seq = 0usize;
            while !stop.load(Ordering::Relaxed) || seq < 10 {
                let rel = reader.call("config.reload", json!({}));
                assert!(rel["error"].is_null(), "full reload failed: {rel}");
                let cfg = reader.call("config.get", json!({}));
                assert!(cfg["error"].is_null(), "{cfg}");
                let ver = assert_version_consistent(&cfg["result"], true).unwrap();
                seq += 1;
                let run = reader.call("commands.run", json!({ "name": "stable:stable_check" }));
                assert!(run["error"].is_null(), "{run}");
                let text = reader.wait_markdown("stable_check", &format!("{ver}:"));
                assert_eq!(text, format!("{ver}:{ver}:{ver}"));
                reloads.fetch_add(1, Ordering::Relaxed);
                if stop.load(Ordering::Relaxed) && seq >= 10 {
                    break;
                }
            }
            reader
        })
    };

    let startup_reader = {
        let (proj, fake_home, tuse_home, url, stop, startups) = (
            proj.clone(),
            fake_home.clone(),
            tuse_home.clone(),
            url.clone(),
            stop.clone(),
            startups.clone(),
        );
        std::thread::spawn(move || {
            let mut count = 0usize;
            while !stop.load(Ordering::Relaxed) || count < 6 {
                let mut p = RpcProc::spawn(&proj, &fake_home, &tuse_home, &url, &[]);
                let cfg = p.call("config.get", json!({}));
                assert!(cfg["error"].is_null(), "{cfg}");
                let ver = assert_version_consistent(&cfg["result"], true).unwrap();
                let run = p.call("commands.run", json!({ "name": "stable:stable_check" }));
                assert!(run["error"].is_null(), "{run}");
                let text = p.wait_markdown("stable_check", &format!("{ver}:"));
                assert_eq!(text, format!("{ver}:{ver}:{ver}"));
                p.shutdown();
                count += 1;
                startups.fetch_add(1, Ordering::Relaxed);
                if stop.load(Ordering::Relaxed) && count >= 6 {
                    break;
                }
            }
        })
    };

    let writer_stable = {
        let (proj, fake_home, tuse_home, url) = (
            proj.clone(),
            fake_home.clone(),
            tuse_home.clone(),
            url.clone(),
        );
        std::thread::spawn(move || {
            let mut w = RpcProc::spawn(&proj, &fake_home, &tuse_home, &url, &["--no-plugins"]);
            for i in 0..16 {
                let src = if i % 2 == 0 {
                    "acme/stable@v2"
                } else {
                    "acme/stable"
                };
                let r = w.call("plugins.add", json!({ "source": src, "replace": true }));
                assert!(r["error"].is_null(), "{r}");
                let u = w.call("plugins.update", json!({ "target": "stable" }));
                assert!(u["error"].is_null(), "{u}");
            }
            w.shutdown();
        })
    };

    let writer_churn = {
        let (proj, fake_home, tuse_home, url) = (
            proj.clone(),
            fake_home.clone(),
            tuse_home.clone(),
            url.clone(),
        );
        std::thread::spawn(move || {
            let mut w = RpcProc::spawn(&proj, &fake_home, &tuse_home, &url, &["--no-plugins"]);
            for _ in 0..12 {
                let r1 = w.call("plugins.add", json!({ "source": "acme/churn" }));
                assert!(r1["error"].is_null(), "{r1}");
                let r2 = w.call(
                    "plugins.add",
                    json!({ "source": "acme/churn@v2", "replace": true }),
                );
                assert!(r2["error"].is_null(), "{r2}");
                let r3 = w.call("plugins.update", json!({ "target": "churn" }));
                assert!(r3["error"].is_null(), "{r3}");
                let r4 = w.call("plugins.delete", json!({ "target": "churn" }));
                assert!(r4["error"].is_null(), "{r4}");
            }
            w.shutdown();
        })
    };

    writer_stable.join().unwrap();
    writer_churn.join().unwrap();
    stop.store(true, Ordering::Relaxed);
    let mut reader = reload_reader.join().unwrap();
    startup_reader.join().unwrap();
    assert!(reloads.load(Ordering::Relaxed) >= 10);
    assert!(startups.load(Ordering::Relaxed) >= 6);

    let set_v1 = admin.call(
        "plugins.add",
        json!({ "source": "acme/stable", "replace": true }),
    );
    assert!(set_v1["error"].is_null(), "{set_v1}");
    assert!(reader.call("config.reload", json!({}))["error"].is_null());
    assert_eq!(
        assert_version_consistent(&reader.call("config.get", json!({}))["result"], true).as_deref(),
        Some("1.0.0")
    );

    let set_v2 = admin.call(
        "plugins.add",
        json!({ "source": "acme/stable@v2", "replace": true }),
    );
    assert!(set_v2["error"].is_null(), "{set_v2}");
    assert_eq!(
        assert_version_consistent(&reader.call("config.get", json!({}))["result"], true).as_deref(),
        Some("1.0.0")
    );
    assert!(
        reader.call("commands.run", json!({ "name": "stable:stable_check" }),)["error"].is_null()
    );
    assert_eq!(
        reader.wait_markdown("stable_check", "1.0.0:"),
        "1.0.0:1.0.0:1.0.0"
    );

    std::fs::write(&cfg_path, "return { work = { prompts = { w = 'v2' } } }\n").unwrap();
    let scoped = reader.call("config.reload", json!({ "ns": "work" }));
    assert_eq!(scoped["error"]["code"], -32602, "{scoped}");
    assert_eq!(
        assert_version_consistent(&reader.call("config.get", json!({}))["result"], true).as_deref(),
        Some("1.0.0")
    );

    let full = reader.call("config.reload", json!({}));
    assert!(full["error"].is_null(), "{full}");
    let after_full = reader.call("config.get", json!({}))["result"].clone();
    assert_eq!(
        assert_version_consistent(&after_full, true).as_deref(),
        Some("2.0.0")
    );
    assert!(
        reader.call("commands.run", json!({ "name": "stable:stable_check" }),)["error"].is_null()
    );
    assert_eq!(
        reader.wait_markdown("stable_check", "2.0.0:"),
        "2.0.0:2.0.0:2.0.0"
    );

    assert!(admin.call("config.reload", json!({}))["error"].is_null());
    assert_eq!(
        admin.call("config.get", json!({}))["result"]["plugins"],
        json!([])
    );
    assert_eq!(
        admin.call("plugins.list", json!({}))["result"]
            .as_array()
            .unwrap()
            .len(),
        1
    );

    let sm = admin.call("plugins.add", json!({ "source": "acme/selfmut" }));
    assert!(sm["error"].is_null(), "{sm}");
    let md = admin.call("plugins.add", json!({ "source": "acme/churn" }));
    assert!(md["error"].is_null(), "{md}");
    std::fs::write(
        &cfg_path,
        r#"
local t = require("tuse")
assert(t.plugins.delete("churn").name == "churn")
return {
  work = { prompts = { w = "v3" } },
}
"#,
    )
    .unwrap();
    let sm_rel = reader.call("config.reload", json!({}));
    assert!(sm_rel["error"].is_null(), "{sm_rel}");
    let sm_cfg = reader.call("config.get", json!({}))["result"].clone();
    assert_eq!(sm_cfg["prompts"]["selfmut:selfmut"], "1.0.0:1.0.0");
    assert!(sm_cfg["prompts"]["churn:churn"].is_null(), "{sm_cfg}");
    assert_eq!(
        admin.call("plugins.list", json!({}))["result"]
            .as_array()
            .unwrap()
            .len(),
        1
    );

    std::fs::write(&cfg_path, "return { work = { prompts = { w = 'v4' } } }\n").unwrap();
    assert!(reader.call("config.reload", json!({}))["error"].is_null());

    let broken = tuse_home.join("plugins/broken");
    std::fs::create_dir_all(&broken).unwrap();
    std::fs::write(broken.join("tuse-plugin.json"), r#"{"version":"1.0.0"}"#).unwrap();

    let broken_start = Command::new(env!("CARGO_BIN_EXE_tuse"))
        .current_dir(&proj)
        .env("HOME", &fake_home)
        .env("TUSE_HOME", &tuse_home)
        .env("TUSE_PLUGINS_URL", &url)
        .env_remove("TUSE_GLOBAL_CONFIG")
        .arg("json")
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(!broken_start.status.success());
    let stderr = String::from_utf8_lossy(&broken_start.stderr);
    assert!(
        stderr.contains("broken") && stderr.contains("entrypoint"),
        "{stderr}"
    );

    let broken_reload = reader.call("config.reload", json!({}));
    let err_msg = broken_reload["error"]["message"].as_str().unwrap_or("");
    assert!(
        err_msg.contains("broken") && err_msg.contains("entrypoint"),
        "{broken_reload}"
    );
    let preserved = reader.call("config.get", json!({}))["result"].clone();
    assert_eq!(
        assert_version_consistent(&preserved, true).as_deref(),
        Some("2.0.0")
    );
    assert_eq!(preserved["prompts"]["work:w"], "v4");

    assert!(admin.call("config.reload", json!({}))["error"].is_null());
    let admin_list = admin.call("plugins.list", json!({}));
    assert!(
        admin_list["error"]["message"]
            .as_str()
            .is_some_and(|m| m.contains("broken")),
        "{admin_list}"
    );

    reader.shutdown();
    admin.shutdown();
    let _ = std::fs::remove_dir_all(&root);
}
#[test]
fn provider_disconnect_in_another_process_is_seen_on_refresh() {
    let root = std::env::temp_dir().join(format!("tuse-provider-refresh-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("tuse.lua"), "return {}").unwrap();
    let home = root.join("state");
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let provider = std::thread::spawn(move || {
        for _ in 0..2 {
            let (mut stream, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut line = String::new();
            while reader.read_line(&mut line).unwrap() > 0 && !line.trim().is_empty() {
                line.clear();
            }
            let body = r#"{"data":[{"id":"one"}]}"#;
            write!(stream,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).unwrap();
        }
    });
    let mut admin = RpcProc::spawn(&root, &root, &home, "", &[]);
    let r = admin.call(
        "providers.connect",
        json!({"name":"remote", "base":base,"key":"mock"}),
    );
    assert_eq!(r["result"]["models"], 1, "{r}");
    let mut reader = RpcProc::spawn(&root, &root, &home, "", &[]);
    let r = reader.call("model.set", json!({"model":"remote/one"}));
    assert!(r["error"].is_null(), "{r}");
    reader.call("model.list", json!({}));
    let end = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while reader.call("model.list", json!({"cached":true}))["result"] != json!(["remote/one"]) {
        assert!(std::time::Instant::now() < end);
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert!(admin.call("providers.disconnect", json!({"name":"remote"}))["error"].is_null());
    assert_eq!(reader.call("model.list", json!({}))["result"], json!([]));
    let state = reader.call("session.state", json!({}));
    assert_eq!(state["result"]["model"], "remote/one");
    assert_eq!(state["result"]["model_info"]["available"], false);
    reader.shutdown();
    admin.shutdown();
    provider.join().unwrap();
    let _ = std::fs::remove_dir_all(root);
}
