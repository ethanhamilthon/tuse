#[path = "../../core/tests/common/mod.rs"]
mod common;
mod support;

use flate2::Compression;
use flate2::write::GzEncoder;
use serde_json::json;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use support::{call, event, spawn};
use tuse_core::{LoadFlags, Mode, Options, Session, Start};

fn mock_provider() -> String {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", l.local_addr().unwrap());
    std::thread::spawn(move || {
        for mut s in l.incoming().flatten() {
            let mut r = BufReader::new(s.try_clone().unwrap());
            let mut line = String::new();
            let mut auth = String::new();
            while r.read_line(&mut line).unwrap_or(0) > 0 && !line.trim().is_empty() {
                if line.to_ascii_lowercase().starts_with("authorization:") {
                    auth = line.clone();
                }
                line.clear();
            }
            let (status, body) = if auth.contains("sk-bad-rpc") {
                ("401 Unauthorized", r#"{"error":"bad key sk-bad-rpc"}"#)
            } else {
                ("200 OK", r#"{"data":[{"id":"m1"},{"id":"m2"}]}"#)
            };
            let _ = write!(
                s,
                "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
        }
    });
    url
}

fn serve_plugin(name: &str, version: &str) -> String {
    let mut enc = GzEncoder::new(Vec::new(), Compression::default());
    {
        let mut tar = tar::Builder::new(&mut enc);
        for (path, body) in [
            (
                "r/tuse-plugin.json",
                format!(r#"{{"name":"{name}","version":"{version}"}}"#),
            ),
            (
                "r/init.lua",
                format!(r#"return {{ prompts = {{ {name} = "v{version}" }} }}"#),
            ),
        ] {
            let mut h = tar::Header::new_gnu();
            h.set_size(body.len() as u64);
            h.set_mode(0o644);
            h.set_cksum();
            tar.append_data(&mut h, path, body.as_bytes()).unwrap();
        }
        tar.finish().unwrap();
    }
    let body = enc.finish().unwrap();
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", l.local_addr().unwrap());
    std::thread::spawn(move || {
        for mut s in l.incoming().flatten() {
            let mut r = BufReader::new(s.try_clone().unwrap());
            let mut line = String::new();
            while r.read_line(&mut line).unwrap_or(0) > 0 && !line.trim().is_empty() {
                line.clear();
            }
            let hdr = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/gzip\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            let _ = s.write_all(hdr.as_bytes());
            let _ = s.write_all(&body);
        }
    });
    url
}

#[test]
fn rpc_providers_and_plugins_parity_redaction_disconnect_and_reload_timing() {
    let prov_url = mock_provider();
    let plug_v1 = serve_plugin("rpcplug", "1.0.0");
    let plug_v2 = serve_plugin("rpcplug", "2.0.0");
    unsafe {
        std::env::set_var("TUSE_PLUGINS_URL", &plug_v1);
    }

    let (dir, cfg) = common::setup(
        "rpc-manage",
        "localhost:1",
        r#"
  commands = {
    lua_check = function()
      local t = require("tuse")
      local ps = t.providers.list()
      local pls = t.plugins.list()
      t.ui.show("parity", tostring(#ps) .. ":" .. tostring(#pls), false)
    end,
  },
"#,
    );
    let session = Session::open({
        unsafe {
            std::env::set_var("TUSE_GLOBAL_CONFIG", cfg.clone());
        }
        Options {
            cwd: dir.clone(),
            mode: Mode::Json,
            resume: None,
            start: Start::Launch,
            flags: LoadFlags::default(),
        }
    })
    .unwrap();
    let (tx, out, server) = spawn(session);

    let init = call(&tx, &out, 1, "initialize", json!({}))["result"].clone();
    let methods: Vec<&str> = init["capabilities"]["methods"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|v| v.as_str())
        .collect();
    for m in [
        "providers.list",
        "providers.connect",
        "providers.update",
        "providers.disconnect",
        "plugins.list",
        "plugins.get",
        "plugins.add",
        "plugins.delete",
        "plugins.update",
    ] {
        assert!(methods.contains(&m), "missing {m}");
    }

    assert_eq!(
        call(&tx, &out, 2, "providers.connect", json!({"name": 123}))["error"]["code"],
        -32602
    );
    assert_eq!(
        call(&tx, &out, 3, "providers.list", json!({"extra": true}))["error"]["code"],
        -32602
    );
    assert_eq!(
        call(
            &tx,
            &out,
            4,
            "plugins.add",
            json!({"source": "a/b", "replace": "yes"})
        )["error"]["code"],
        -32602
    );
    assert_eq!(
        call(
            &tx,
            &out,
            5,
            "providers.disconnect",
            json!({"name": "nope"})
        )["error"]["code"],
        -32000
    );
    assert_eq!(
        call(&tx, &out, 6, "plugins.delete", json!({"target": "nope"}))["error"]["code"],
        -32000
    );

    let bad_probe = call(
        &tx,
        &out,
        7,
        "providers.connect",
        json!({
            "name": "badp",
            "api": "openai-chat",
            "base": prov_url,
            "key": "sk-bad-rpc",
        }),
    );
    assert_eq!(bad_probe["error"]["code"], -32000);
    assert!(!bad_probe.to_string().contains("sk-bad-rpc"));

    let connected = call(
        &tx,
        &out,
        8,
        "providers.connect",
        json!({
            "name": "live",
            "api": "openai-chat",
            "base": prov_url,
            "key": "sk-good-secret",
            "headers": {"X-Token": "hdr-secret"},
        }),
    );
    assert_eq!(connected["result"]["name"], "live");
    assert_eq!(connected["result"]["type"], "openai-chat");
    assert_eq!(connected["result"]["models"], 2);
    assert!(!connected.to_string().contains("sk-good-secret"));
    assert!(!connected.to_string().contains("hdr-secret"));

    let plist = call(&tx, &out, 9, "providers.list", json!({}))["result"].clone();
    assert!(!plist.to_string().contains("sk-good-secret"));
    assert!(!plist.to_string().contains("hdr-secret"));
    assert!(
        plist
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v["name"] == "live" && v["type"] == "openai-chat" && v["base"] == prov_url)
    );

    let updated = call(
        &tx,
        &out,
        10,
        "providers.update",
        json!({"name": "live", "api": "openai-responses"}),
    );
    assert_eq!(updated["result"]["type"], "openai-responses");

    assert!(
        call(
            &tx,
            &out,
            11,
            "model.set",
            json!({"model": "live/m1", "effort": "medium"})
        )["error"]
            .is_null()
    );
    let before_disc = call(&tx, &out, 12, "session.state", json!({}))["result"].clone();
    assert_eq!(before_disc["model"], "live/m1");
    assert_eq!(before_disc["model_info"]["available"], true);

    let disc = call(
        &tx,
        &out,
        13,
        "providers.disconnect",
        json!({"name": "live"}),
    );
    assert_eq!(disc["result"], json!({"name": "live"}));

    let after_disc = call(&tx, &out, 14, "session.state", json!({}))["result"].clone();
    assert_eq!(after_disc["model"], "live/m1");
    assert_eq!(after_disc["provider"], "live");
    assert_eq!(after_disc["model_info"]["available"], false);

    call(&tx, &out, 15, "turn.submit", json!({"text": "should fail"}));
    let err_ev = event(&out, "error");
    assert!(
        err_ev["text"].as_str().unwrap().contains("not connected"),
        "{err_ev}"
    );
    event(&out, "turn_end");

    let added = call(
        &tx,
        &out,
        16,
        "plugins.add",
        json!({"source": "acme/rpcplug"}),
    )["result"]
        .clone();
    assert_eq!(added["name"], "rpcplug");
    assert_eq!(added["version"], "1.0.0");
    assert_eq!(added["replaced"], false);

    let pl_list = call(&tx, &out, 17, "plugins.list", json!({}))["result"].clone();
    assert_eq!(pl_list.as_array().unwrap().len(), 1);
    let pl_get = call(&tx, &out, 18, "plugins.get", json!({"target": "rpcplug"}))["result"].clone();
    assert_eq!(pl_get["manifest"]["name"], "rpcplug");

    assert!(
        call(&tx, &out, 19, "config.get", json!({}))["result"]["prompts"]["rpcplug:rpcplug"]
            .is_null()
    );
    assert!(call(&tx, &out, 20, "config.reload", json!({}))["error"].is_null());
    event(&out, "reloaded");
    assert_eq!(
        call(&tx, &out, 21, "config.get", json!({}))["result"]["prompts"]["rpcplug:rpcplug"],
        "v1.0.0"
    );

    unsafe {
        std::env::set_var("TUSE_PLUGINS_URL", &plug_v2);
    }
    let upd = call(
        &tx,
        &out,
        22,
        "plugins.update",
        json!({"target": "rpcplug"}),
    )["result"]
        .clone();
    assert_eq!(upd["updated"][0]["version"], "2.0.0");
    assert_eq!(
        call(&tx, &out, 23, "config.get", json!({}))["result"]["prompts"]["rpcplug:rpcplug"],
        "v1.0.0"
    );
    assert!(call(&tx, &out, 24, "config.reload", json!({}))["error"].is_null());
    assert_eq!(
        call(&tx, &out, 25, "config.get", json!({}))["result"]["prompts"]["rpcplug:rpcplug"],
        "v2.0.0"
    );

    call(
        &tx,
        &out,
        26,
        "commands.run",
        json!({"name": "test:lua_check"}),
    );
    let md = support::until(&out, |v| {
        v["method"] == "event"
            && v["params"]["type"] == "ui_markdown"
            && v["params"]["name"] == "parity"
    });
    assert_eq!(md["params"]["text"], "1:1");

    let del = call(
        &tx,
        &out,
        27,
        "plugins.delete",
        json!({"target": "rpcplug"}),
    )["result"]
        .clone();
    assert_eq!(del, json!({"name": "rpcplug"}));
    assert_eq!(
        call(&tx, &out, 28, "plugins.list", json!({}))["result"]
            .as_array()
            .unwrap()
            .len(),
        0
    );
    assert_eq!(
        call(&tx, &out, 29, "config.get", json!({}))["result"]["prompts"]["rpcplug:rpcplug"],
        "v2.0.0"
    );
    assert!(call(&tx, &out, 30, "config.reload", json!({}))["error"].is_null());
    assert!(
        call(&tx, &out, 31, "config.get", json!({}))["result"]["prompts"]["rpcplug:rpcplug"]
            .is_null()
    );

    call(&tx, &out, 32, "shutdown", json!({}));
    server.join().unwrap();
    unsafe {
        std::env::remove_var("TUSE_PLUGINS_URL");
    }
    common::cleanup(&dir);
}
