use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::path::Path;
use tuse_config::{ModelDef, Prices};
use tuse_core::{Options, Session};

static ENV: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn mock_models(body: &'static str) -> String {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", l.local_addr().unwrap());
    std::thread::spawn(move || {
        for mut s in l.incoming().flatten() {
            let mut r = BufReader::new(s.try_clone().unwrap());
            let mut line = String::new();
            while r.read_line(&mut line).unwrap_or(0) > 0 && !line.trim().is_empty() {
                line.clear();
            }
            let _ = write!(
                s,
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
        }
    });
    url
}

fn open(cfg: &Path, cwd: &Path, resume: Option<String>) -> std::sync::Arc<Session> {
    Session::open({
        unsafe {
            std::env::set_var("TUSE_GLOBAL_CONFIG", cfg);
        }
        Options {
            cwd: cwd.to_path_buf(),
            mode: tuse_core::Mode::Json,
            resume,
            start: Default::default(),
            flags: Default::default(),
        }
    })
    .unwrap()
}

#[test]
fn a_session_keeps_its_selection_and_reloads_model_metadata() {
    let _g = ENV.lock().unwrap_or_else(|e| e.into_inner());
    let dir = std::env::temp_dir().join(format!("tuse-model-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    let home = dir.join("home");
    std::fs::create_dir_all(&home).unwrap();
    unsafe {
        std::env::set_var("HOME", &home);
        std::env::set_var("TUSE_HOME", home.join(".tuse"));
    }
    let url = mock_models(r#"{"data":[{"id":"a"},{"id":"discovered_only"},{"id":"no_reason"}]}"#);
    tuse_core::Providers::add(&tuse_config::Connect {
        name: "p".into(),
        base_url: url,
        api_key: "k".into(),
        ..Default::default()
    })
    .unwrap();
    let cfg = dir.join("tuse.lua");
    let models = dir.join("models.lua");
    std::fs::write(&cfg, "return { models = require('models') }").unwrap();
    std::fs::write(
        &models,
        r#"return {
            ["p/a"] = {
                display = "Model A",
                window = 200000,
                prices = { input = 3, output = 15, cache_read = 0.3, cache_write = 3.75 },
            },
            ["p/meta_only"] = {
                display = "Metadata Only",
                window = 64000,
            },
            ["p/no_reason"] = {
                display = "No Reasoning",
                efforts = {},
            },
        }"#,
    )
    .unwrap();
    tuse_core::Prefs::set("model", "p/a").unwrap();
    tuse_core::Prefs::set("effort", "high").unwrap();
    let s = open(&cfg, &dir, None);
    assert_eq!(s.model(), ("p/a".into(), "high".into()));
    assert!(s.available_models().is_empty());
    let ev = s.subscribe();
    s.fetch_models();
    while !matches!(
        ev.recv_timeout(std::time::Duration::from_secs(5)),
        Ok(tuse_core::Event::Models) | Err(_)
    ) {}
    assert_eq!(
        s.available_models(),
        ["p/a", "p/discovered_only", "p/no_reason"]
    );
    assert_eq!(s.model_def("p/meta_only").unwrap().display, "Metadata Only");
    assert_eq!(
        s.config().model_efforts("p/no_reason"),
        Vec::<String>::new()
    );
    assert_eq!(
        s.config().model_efforts("p/discovered_only"),
        tuse_config::DEFAULT_EFFORTS
    );
    assert_eq!(s.snapshot(false).context_window, Some(200_000));
    assert_eq!(
        s.model_def("p/a"),
        Some(ModelDef {
            display: "Model A".into(),
            window: 200_000,
            efforts: None,
            prices: Some(Prices {
                input: 3.0,
                output: 15.0,
                cache_read: 0.3,
                cache_write: 3.75,
            }),
        })
    );
    let id = s.id.clone();
    s.select_model("p/no_reason", "", false).unwrap();
    assert_eq!(s.model(), ("p/no_reason".into(), "".into()));
    s.select_model("p/a", "high", false).unwrap();
    let events = s.subscribe();
    s.submit(tuse_core::UserInput::text("hi"));
    while !matches!(
        events.recv_timeout(std::time::Duration::from_secs(10)),
        Ok(tuse_core::Event::TurnEnd { .. }) | Err(_)
    ) {}
    tuse_core::Prefs::set("model", "p/b").unwrap();
    tuse_core::Prefs::set("effort", "low").unwrap();
    std::fs::write(&models, "return { ['p/a'] = {window=300000} }").unwrap();
    s.reload().unwrap();
    assert_eq!(s.model(), ("p/a".into(), "high".into()));
    assert_eq!(s.snapshot(false).context_window, Some(300_000));
    s.close();
    let again = open(&cfg, &dir, Some(id.clone()));
    assert_eq!(again.model(), ("p/a".into(), "high".into()));
    again.close();
    let fresh = open(&cfg, &dir, None);
    assert_eq!(fresh.model(), ("p/b".into(), "low".into()));
    fresh.close();
    assert!(home.join(".tuse").join(&fresh.id).exists());
    assert_eq!(fresh.snapshot(false).started, Some(false));
    assert!(tuse_core::Meta::list(&dir).iter().all(|m| m.id != fresh.id));
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn session_set_effort_fallback_and_inflight_disconnect_behavior() {
    let dir = std::env::temp_dir().join(format!("tuse-model-disc-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    let home = dir.join("home");
    std::fs::create_dir_all(&home).unwrap();
    let _g = ENV.lock().unwrap_or_else(|e| e.into_inner());
    unsafe {
        std::env::set_var("TUSE_HOME", &home);
    }

    let (arrived_tx, arrived_rx) = std::sync::mpsc::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let release_rx = std::sync::Arc::new(std::sync::Mutex::new(release_rx));
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", l.local_addr().unwrap());
    let rel = release_rx.clone();
    std::thread::spawn(move || {
        for mut s in l.incoming().flatten() {
            let mut r = BufReader::new(s.try_clone().unwrap());
            let mut first = String::new();
            let _ = r.read_line(&mut first);
            let mut line = String::new();
            while r.read_line(&mut line).unwrap_or(0) > 0 && !line.trim().is_empty() {
                line.clear();
            }
            if first.starts_with("GET ") {
                let body = r#"{"data":[{"id":"plain"},{"id":"reason"},{"id":"highonly"}]}"#;
                let _ = write!(
                    s,
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
            } else {
                let _ = arrived_tx.send(());
                let _ = rel
                    .lock()
                    .unwrap()
                    .recv_timeout(std::time::Duration::from_secs(5));
                let _ = write!(
                    s,
                    "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n\
                     data: {{\"choices\":[{{\"delta\":{{\"content\":\"in-flight-ok\"}}}}]}}\n\n\
                     data: [DONE]\n\n"
                );
            }
        }
    });

    tuse_core::Providers::connect(tuse_core::ConnectInput {
        name: "live".into(),
        api: Some("openai-chat".into()),
        base: url,
        key: "k-live".into(),
        headers: None,
        replace: false,
    })
    .unwrap();
    let cfg = dir.join("tuse.lua");
    std::fs::write(
        &cfg,
        r#"local t = require("tuse")
        return {
            models = {
                ["live/plain"] = { efforts = {} },
                ["live/reason"] = { },
                ["live/highonly"] = { efforts = { "high", "low" } },
            },
            test = { commands = {
                reason = function() t.session.set{model="live/reason"} end,
                plain = function() t.session.set{model="live/plain"} end,
                highonly = function() t.session.set{model="live/highonly"} end,
                call_llm = function()
                    local ok, err = pcall(t.llm, "hi")
                    t.ui.show("llm_res", tostring(ok) .. ":" .. tostring(err), false)
                end,
            },
        } }"#,
    )
    .unwrap();
    let s = open(&cfg, &dir, None);
    let ev = s.subscribe();
    s.fetch_models();
    while !matches!(
        ev.recv_timeout(std::time::Duration::from_secs(5)),
        Ok(tuse_core::Event::Models) | Err(_)
    ) {}
    assert_eq!(
        s.available_models(),
        ["live/highonly", "live/plain", "live/reason"]
    );

    s.select_model("live/plain", "", false).unwrap();
    assert_eq!(s.model(), ("live/plain".into(), "".into()));

    s.run_command("test:reason", &[]).unwrap();
    let dl = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while s.model().0 != "live/reason" && std::time::Instant::now() < dl {
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert_eq!(s.model(), ("live/reason".into(), "medium".into()));

    s.run_command("test:plain", &[]).unwrap();
    let dl = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while s.model().0 != "live/plain" && std::time::Instant::now() < dl {
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert_eq!(s.model(), ("live/plain".into(), "".into()));

    s.run_command("test:highonly", &[]).unwrap();
    let dl = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while s.model().0 != "live/highonly" && std::time::Instant::now() < dl {
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert_eq!(s.model(), ("live/highonly".into(), "high".into()));

    s.submit(tuse_core::UserInput::text("start"));
    arrived_rx
        .recv_timeout(std::time::Duration::from_secs(5))
        .unwrap();

    s.disconnect_provider("live").unwrap();
    assert_eq!(s.model(), ("live/highonly".into(), "high".into()));
    assert!(!s.snapshot(false).model_info.unwrap().available);
    assert!(s.available_models().is_empty());

    release_tx.send(()).unwrap();
    let mut got_inflight = false;
    loop {
        match ev.recv_timeout(std::time::Duration::from_secs(5)).unwrap() {
            tuse_core::Event::Assistant { text } if text.contains("in-flight-ok") => {
                got_inflight = true;
            }
            tuse_core::Event::TurnEnd { .. } => break,
            _ => {}
        }
    }
    assert!(got_inflight);

    s.submit(tuse_core::UserInput::text("after disconnect"));
    let mut got_err = String::new();
    loop {
        match ev.recv_timeout(std::time::Duration::from_secs(5)).unwrap() {
            tuse_core::Event::Error(e) => got_err = e,
            tuse_core::Event::TurnEnd { .. } => break,
            _ => {}
        }
    }
    assert!(got_err.contains("live"), "{got_err}");

    s.run_command("test:call_llm", &[]).unwrap();
    let dl = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while std::time::Instant::now() < dl {
        if s.ui_snapshot()
            .markdowns
            .iter()
            .any(|m| m.name == "llm_res")
        {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let snap = s.ui_snapshot();
    let llm_text = &snap
        .markdowns
        .iter()
        .find(|m| m.name == "llm_res")
        .unwrap()
        .text;
    assert!(
        llm_text.starts_with("false:") && llm_text.contains("live"),
        "{llm_text}"
    );

    s.close();
    unsafe {
        std::env::remove_var("TUSE_HOME");
    }
    std::fs::remove_dir_all(dir).ok();
}
