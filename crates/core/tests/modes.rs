mod common;
use common::Reply;
use serde_json::json;
use std::time::{Duration, Instant};
use tuse_core::{Event, Mode, Options, Record, Session, UserInput};

fn wait_turn(events: &std::sync::mpsc::Receiver<Event>) -> Option<String> {
    let start = Instant::now();
    let mut err = None;
    while start.elapsed() < Duration::from_secs(30) {
        match events.recv_timeout(Duration::from_millis(50)) {
            Ok(Event::Error(e)) => err = Some(e),
            Ok(Event::TurnEnd { .. }) => return err,
            _ => {}
        }
    }
    panic!("the turn did not end");
}

#[test]
fn modes_filter_systems_functions_connect_and_includes() {
    let code = r#"
print("all=" + str(f_all()))
print("hl=" + str(f_hl()))
try:
    f_tui()
    print("tui=ok")
except Exception as e:
    print("tui=missing:" + str(e))
print("docs_has_hl=" + str("`f_hl(" in docs("functions")))
print("docs_has_tui=" + str("`f_tui(" in docs("functions")))
"#;
    let addr = common::provider(vec![
        Reply::Call("python", json!({"code": code, "title": "t"}).to_string()),
        Reply::Text("done"),
        Reply::Text("tui ok"),
    ]);
    let (dir, _) = common::setup("modes", &addr, "");
    std::fs::write(
        dir.join("inc.yaml"),
        "systems: [from-tui-include]\neffort: high\n",
    )
    .unwrap();
    let cfg = dir.join("main.yaml");
    std::fs::write(
        &cfg,
        format!(
            r#"include:
  - file: inc.yaml
    only: [systems]
    modes: [tui]
model: p/m
connect:
  - {{name: p, base_url: 'http://{addr}', api_key: k, modes: [json, tui]}}
  - {{name: tui_gw, base_url: 'http://{addr}', api_key: k, modes: [tui]}}
systems:
  - plain-sys
  - {{text: tui-sys, modes: [tui]}}
  - {{text: hl-sys, modes: [json]}}
functions:
  f_all: {{description: all, run: "return 1"}}
  f_hl: {{description: hl, run: "return 2", modes: [json]}}
  f_tui: {{description: tui, run: "return 3", modes: [tui]}}
"#
        ),
    )
    .unwrap();
    let open = |mode| {
        Session::open(Options {
            cwd: dir.clone(),
            config: Some(cfg.clone()),
            mode,
            resume: None,
            start: Default::default(),
        })
        .unwrap()
    };

    // Json: gets plain-sys + hl-sys + f_all + f_hl, excludes tui include/sys/func.
    let hl = open(Mode::Json);
    let sys = &hl.trace()[0].text;
    assert!(sys.contains("plain-sys") && sys.contains("hl-sys"), "{sys}");
    assert!(
        !sys.contains("tui-sys") && !sys.contains("from-tui-include"),
        "{sys}"
    );
    assert!(sys.contains("`f_all(") && sys.contains("`f_hl(") && !sys.contains("f_tui"));

    let events = hl.subscribe();
    hl.submit(UserInput::text("check"));
    assert_eq!(wait_turn(&events), None);
    let out = hl
        .records()
        .into_iter()
        .find_map(|r| match r {
            Record::Tool { output, .. } => Some(output),
            _ => None,
        })
        .expect("no tool call");
    for want in [
        "all=1",
        "hl=2",
        "tui=missing",
        "docs_has_hl=True",
        "docs_has_tui=False",
    ] {
        assert!(out.contains(want), "missing `{want}` in:\n{out}");
    }

    // Switching to a tui-only connect in json fails the turn cleanly.
    hl.set_model("tui_gw/m", "default");
    hl.submit(UserInput::text("fail"));
    let err = wait_turn(&events).expect("expected connect error");
    assert!(err.contains("no `connect` entry named `tui_gw`"), "{err}");
    hl.close();

    // Tui: gets tui include (only systems, not effort), tui-sys, f_tui, and tui_gw works.
    // fetch_models only queries connect entries matching the active mode.
    let m_listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let m_addr = m_listener.local_addr().unwrap();
    std::thread::spawn(move || {
        use std::io::{BufRead, BufReader, Write};
        for mut s in m_listener.incoming().flatten() {
            let mut r = BufReader::new(s.try_clone().unwrap());
            let mut line = String::new();
            while r.read_line(&mut line).unwrap_or(0) > 0 && !line.trim().is_empty() {
                line.clear();
            }
            let body = r#"{"data":[{"id":"m1"}]}"#;
            let _ = write!(
                s,
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = s.flush();
        }
    });
    let m_cfg = dir.join("models.yaml");
    std::fs::write(
        &m_cfg,
        format!(
            "connect:\n  - {{name: tui_p, base_url: 'http://{m_addr}', api_key: k, modes: [tui]}}\n  - {{name: hl_p, base_url: 'http://{m_addr}', api_key: k, modes: [json]}}\n"
        ),
    )
    .unwrap();
    let m_hl = Session::open(Options {
        cwd: dir.clone(),
        config: Some(m_cfg),
        mode: Mode::Json,
        resume: None,
        start: Default::default(),
    })
    .unwrap();
    let m_ev = m_hl.subscribe();
    m_hl.fetch_models();
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(5) {
        if matches!(
            m_ev.recv_timeout(Duration::from_millis(50)),
            Ok(Event::Models)
        ) {
            break;
        }
    }
    assert_eq!(m_hl.remote_models(), ["hl_p/m1"]);
    m_hl.close();

    let tui = open(Mode::Tui);
    assert_eq!(tui.config().effort, "");
    let sys = &tui.trace()[0].text;
    assert!(
        sys.contains("plain-sys") && sys.contains("tui-sys") && sys.contains("from-tui-include")
    );
    assert!(!sys.contains("hl-sys"));
    assert!(sys.contains("`f_tui(") && !sys.contains("f_hl"));
    let events = tui.subscribe();
    tui.set_model("tui_gw/m", "default");
    tui.submit(UserInput::text("hi"));
    assert_eq!(wait_turn(&events), None);
    tui.close();

    common::cleanup(&dir);
}
