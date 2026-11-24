#[path = "../common/mod.rs"]
mod common;
use common::Reply;
use std::time::{Duration, Instant};
use tuse_core::{Event, Mode, Options, Session, Start, UserInput};

fn config(tag: &str) -> String {
    format!(
        r#"return {{
  prompts = {{ tag = "{tag}" }},
  commands = {{ again = function() require("tuse").reload() end }},
  hooks = {{
    on_user = {{ events = {{ "user_message" }}, run = function() require("tuse").reload() end }},
  }},
}}"#
    )
}

fn wait_tag(s: &Session, tag: &str) {
    let end = Instant::now() + Duration::from_secs(5);
    while s.config().prompts["tag"] != tag {
        assert!(Instant::now() < end, "still {}", s.config().prompts["tag"]);
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn reload_from_the_agent_commands_and_hooks() {
    let code = "local before = session().configpath; reload(); return 'reloaded'";
    let addr = common::provider(vec![
        Reply::Call("lua", serde_json::json!({ "code": code }).to_string()),
        Reply::Text("ok"),
    ]);
    let (dir, cfg) = common::setup("lua-reload", &addr, "");
    std::fs::write(&cfg, config("v1")).unwrap();
    let s = Session::open(Options {
        cwd: dir.clone(),
        config: Some(cfg.clone()),
        mode: Mode::Json,
        resume: None,
        start: Start::Launch,
    })
    .unwrap();
    let rx = s.subscribe();

    std::fs::write(&cfg, config("v2")).unwrap();
    s.run_command("again").unwrap();
    wait_tag(&s, "v2");

    std::fs::write(&cfg, config("v3").replace("on_user", "on_user2")).unwrap();
    s.submit(UserInput::text("go"));
    let mut output = String::new();
    loop {
        match rx
            .recv_timeout(Duration::from_secs(10))
            .expect("turn ended")
        {
            Event::ToolEnd { output: o, .. } => output = o,
            Event::TurnEnd { .. } => break,
            _ => {}
        }
    }
    assert!(output.contains("reloaded"), "{output}");
    wait_tag(&s, "v3");

    std::fs::write(&cfg, config("v4")).unwrap();
    s.submit(UserInput::text("hook only"));
    wait_tag(&s, "v4");

    std::fs::write(&cfg, "return { typo_key = 1 }").unwrap();
    s.run_command("again").unwrap();
    let end = Instant::now() + Duration::from_secs(5);
    loop {
        match rx.recv_timeout(end - Instant::now()).expect("an error") {
            Event::Error(e) if e.contains("typo_key") => break,
            _ => {}
        }
    }
    assert_eq!(s.config().prompts["tag"], "v4");
    s.close();
    common::cleanup(&dir);
}
