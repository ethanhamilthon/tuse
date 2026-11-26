#[path = "../common/mod.rs"]
mod common;

use common::Reply;
use serde_json::json;
use std::time::{Duration, Instant};
use tuse_core::{Event, Mode, Options, Session, Start, UserInput};

fn wait_turn(rx: &std::sync::mpsc::Receiver<Event>) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        if matches!(
            rx.recv_timeout(Duration::from_millis(50)),
            Ok(Event::TurnEnd { .. })
        ) {
            return;
        }
    }
    panic!("turn did not end");
}

#[test]
fn session_snapshot_history_and_open_handlers() {
    let long_arg = "x".repeat(400);
    let check_code = r#"
local t = require("tuse")
local s1 = t.session()
s1.name = "mutated"
local s2 = t.session()
assert(s2.name ~= "mutated", "session() must return a fresh snapshot")
assert(s2.mode == "json" and s2.id ~= nil and s2.cwd ~= nil and s2.home ~= nil and s2.configpath ~= nil)
assert(s2.model == "p/m" and s2.provider == "p" and s2.effort == "default")
assert(s2.ctx_size == 100000 and s2.model_info.display == "M" and s2.model_info.prices.input == 2)
assert(s2.usage.input >= 10 and s2.usage.output >= 3 and s2.usage.cost > 0)
assert(s2.steps >= 1 and s2.turns >= 0)
local dump = t.json.encode(s2)
assert(not dump:find("sk-super-secret", 1, true), "must never expose credentials")
return #s2.history
"#;
    let addr = common::provider(vec![
        Reply::Text("first reply"),
        Reply::Call("lua", json!({"code": check_code}).to_string()),
        Reply::Text("second reply"),
    ]);
    let (dir, cfg) = common::setup("session-open", &addr, "");
    tuse_core::Providers::add(&tuse_config::Connect {
        name: "p".into(),
        base_url: format!("http://{addr}"),
        api_key: "sk-super-secret".into(),
        ..Default::default()
    })
    .unwrap();
    let opened_log = dir.join("opened.txt");
    let sample = dir.join("notes/README.MD");
    std::fs::create_dir_all(sample.parent().unwrap()).unwrap();
    std::fs::write(&sample, "# hi").unwrap();
    std::fs::write(
        &cfg,
        format!(
            r#"local function rec(tag)
  return function(path)
    local f = assert(io.open("{}", "a"))
    f:write(tag, " ", path, "\n")
    f:close()
  end
end
return {{
  models = {{{{ name = "p/m", display = "M", window = 100000,
    prices = {{ input = 2, output = 10, cache_read = 0.2, cache_write = 2.5 }} }}}},
  open = {{
    md = rec("md"),
    ["*.note"] = rec("mask"),
    ["*"] = rec("any"),
    ["@dir"] = rec("dir"),
    ["@url"] = rec("url"),
    bad = function() error("opener crashed") end,
  }},
  hooks = {{
    keep = {{
      events = {{"user_message"}},
      run = function() end,
    }},
  }},
}}"#,
            opened_log.display()
        ),
    )
    .unwrap();

    let s1 = Session::open(Options {
        cwd: dir.clone(),
        config: Some(cfg.clone()),
        mode: Mode::Json,
        resume: None,
        start: Start::Launch,
    })
    .unwrap();
    let rx1 = s1.subscribe();
    s1.submit(UserInput::text(long_arg.clone()));
    wait_turn(&rx1);
    let id = s1.id.clone();
    s1.close();

    let s2 = Session::open(Options {
        cwd: dir.clone(),
        config: Some(cfg.clone()),
        mode: Mode::Json,
        resume: Some(id),
        start: Start::Resume,
    })
    .unwrap();
    let rx2 = s2.subscribe();
    s2.submit(UserInput::text("verify"));
    wait_turn(&rx2);

    let snap = s2.snapshot(true);
    assert_eq!(snap.turns, Some(2));
    assert_eq!(snap.steps, Some(3));
    let history = snap.history.as_ref().unwrap();
    assert!(
        history
            .iter()
            .any(|t| t.role == "user" && t.detail == long_arg)
    );
    assert!(
        history.iter().any(|t| t.role == "hook"
            && t.fields["vars"] == serde_json::json!({"event": "user_message"}))
    );
    assert!(
        history
            .iter()
            .any(|t| t.role == "agent" && t.detail == "first reply")
    );
    assert!(
        history
            .iter()
            .any(|t| t.role == "agent" && t.detail == "second reply")
    );

    s2.open_path(std::path::Path::new("notes/README.MD"));
    let dl = Instant::now() + Duration::from_secs(3);
    while Instant::now() < dl && !opened_log.exists() {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(
        std::fs::read_to_string(&opened_log).unwrap().trim(),
        format!("md {}", sample.canonicalize().unwrap().display())
    );

    let note = dir.join("a.NOTE");
    let other = dir.join("b.txt");
    std::fs::write(&note, "x").unwrap();
    std::fs::write(&other, "x").unwrap();
    s2.open_path(&note);
    s2.open_path(&other);
    s2.open_path(&dir.join("notes"));
    s2.open_url("https://example.com/x");
    let dl = Instant::now() + Duration::from_secs(3);
    while Instant::now() < dl
        && std::fs::read_to_string(&opened_log)
            .unwrap()
            .lines()
            .count()
            < 5
    {
        std::thread::sleep(Duration::from_millis(20));
    }
    let log = std::fs::read_to_string(&opened_log).unwrap();
    let canon = |p: &std::path::Path| p.canonicalize().unwrap().display().to_string();
    for want in [
        format!("mask {}", canon(&note)),
        format!("any {}", canon(&other)),
        format!("dir {}", canon(&dir.join("notes"))),
        "url https://example.com/x".to_string(),
    ] {
        assert!(log.lines().any(|l| l == want), "{want}\n{log}");
    }

    s2.open_path(std::path::Path::new("notes/missing.md"));
    let bad_file = dir.join("x.bad");
    std::fs::write(&bad_file, "x").unwrap();
    s2.open_path(&bad_file);
    s2.open_url("javascript:alert(1)");
    let mut errs = vec![];
    let dl = Instant::now() + Duration::from_secs(3);
    while Instant::now() < dl && errs.len() < 3 {
        if let Ok(Event::Error(e)) = rx2.recv_timeout(Duration::from_millis(50)) {
            errs.push(e);
        }
    }
    assert!(
        errs.iter().any(|e| e.contains("file not found")),
        "{errs:?}"
    );
    assert!(
        errs.iter()
            .any(|e| e.contains("open.bad") && e.contains("opener crashed")),
        "{errs:?}"
    );
    assert!(
        errs.iter().any(|e| e.contains("cannot open URL")),
        "{errs:?}"
    );

    let old_cost = snap.usage.unwrap().cost.unwrap();
    let original = std::fs::read_to_string(&cfg).unwrap();
    let doubled = original.replace(
        "input = 2, output = 10, cache_read = 0.2, cache_write = 2.5",
        "input = 4, output = 20, cache_read = 0.4, cache_write = 5",
    );
    std::fs::write(&cfg, doubled).unwrap();
    s2.reload().unwrap();
    assert!((s2.stats().cost - old_cost).abs() < 1e-9);
    s2.close();
    common::cleanup(&dir);
}
