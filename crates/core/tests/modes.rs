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
print("all=" .. f_all())
print("hl=" .. f_hl())
print("tui=" .. tostring(f_tui))
print("docs_has_hl=" .. tostring(docs("functions"):find("`f_hl(", 1, true) ~= nil))
print("docs_has_tui=" .. tostring(docs("functions"):find("`f_tui(", 1, true) ~= nil))
"#;
    let addr = common::provider(vec![
        Reply::Call("lua", json!({"code": code}).to_string()),
        Reply::Text("done"),
        Reply::Text("tui ok"),
    ]);
    let (dir, _) = common::setup("modes", &addr, "");
    std::fs::write(
        dir.join("inc.lua"),
        "return { systems = {'from-tui-include'}, effort = 'high' }",
    )
    .unwrap();
    let gw = |name: &str, modes| tuse_config::Connect {
        name: name.into(),
        base_url: format!("http://{addr}"),
        api_key: "k".into(),
        modes,
    };
    tuse_core::add_provider(&gw(
        "p",
        vec![tuse_config::Mode::Json, tuse_config::Mode::Tui],
    ))
    .unwrap();
    tuse_core::add_provider(&gw("tui_gw", vec![tuse_config::Mode::Tui])).unwrap();
    let cfg = dir.join("main.lua");
    std::fs::write(
        &cfg,
        r#"local systems = {"plain-sys"}
if MODE == "tui" then
  systems[#systems + 1] = "tui-sys"
  for _, s in ipairs(require("inc").systems) do
    systems[#systems + 1] = s
  end
end
systems[#systems + 1] = {text = "hl-sys", modes = {"json"}}
return {
  model = "p/m",
  systems = systems,
  functions = {
    f_all = {description = "all", run = function() return 1 end},
    f_hl = {description = "hl", run = function() return 2 end, modes = {"json"}},
    f_tui = {description = "tui", run = function() return 3 end, modes = {"tui"}},
  },
}
"#,
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
        "tui=nil",
        "docs_has_hl=true",
        "docs_has_tui=false",
    ] {
        assert!(out.contains(want), "missing `{want}` in:\n{out}");
    }

    // Switching to a tui-only connect in json fails the turn cleanly.
    hl.set_model("tui_gw/m", "default");
    hl.submit(UserInput::text("fail"));
    let err = wait_turn(&events).expect("expected connect error");
    assert!(err.contains("no `connect` entry named `tui_gw`"), "{err}");
    hl.close();

    // Tui: gets the systems of the required file (not its effort), tui-sys, f_tui, and tui_gw works.
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
    for (name, mode) in [
        ("tui_p", tuse_config::Mode::Tui),
        ("hl_p", tuse_config::Mode::Json),
    ] {
        tuse_core::add_provider(&tuse_config::Connect {
            name: name.into(),
            base_url: format!("http://{m_addr}"),
            api_key: "k".into(),
            modes: vec![mode],
        })
        .unwrap();
    }
    tuse_core::remove_provider("p").unwrap();
    tuse_core::remove_provider("tui_gw").unwrap();
    let m_cfg = dir.join("models.lua");
    std::fs::write(&m_cfg, "return {}").unwrap();
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
    tuse_core::remove_provider("tui_p").unwrap();
    tuse_core::remove_provider("hl_p").unwrap();
    tuse_core::add_provider(&gw("p", vec![])).unwrap();
    tuse_core::add_provider(&gw("tui_gw", vec![tuse_config::Mode::Tui])).unwrap();

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
