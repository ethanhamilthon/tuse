#[path = "../common/mod.rs"]
mod common;
use common::Reply;
use std::time::{Duration, Instant};
use tuse_core::{Event, Mode, Options, Session, Start, UserInput};

fn config(tag: &str) -> String {
    format!(
        r#"return {{
  other = {{ prompts = {{ untouched = "{tag}" }} }},
  test = {{
    prompts = {{ tag = "{tag}" }},
    commands = {{ again = function() require("tuse").reload() end }},
    hook = {{
      user_message = function() require("tuse").reload() end,
    }},
  }},
}}"#
    )
}

fn wait_tag(s: &Session, tag: &str) {
    let end = Instant::now() + Duration::from_secs(5);
    while s.config().prompts["test:tag"] != tag {
        assert!(
            Instant::now() < end,
            "still {}",
            s.config().prompts["test:tag"]
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn reload_from_the_agent_commands_and_hooks() {
    let code = "assert(not pcall(reload, nil)); assert(not pcall(reload, 123)); assert(not pcall(reload, '')); assert(not pcall(reload, 'missing')); reload(); return 'reloaded'";
    let addr = common::provider(vec![
        Reply::Call("lua", serde_json::json!({ "code": code }).to_string()),
        Reply::Text("ok"),
    ]);
    let (dir, cfg) = common::setup("lua-reload", &addr, "");
    std::fs::write(&cfg, config("v1")).unwrap();
    unsafe { std::env::set_var("TUSE_GLOBAL_CONFIG", &cfg) };
    let s = Session::open({
        unsafe {
            std::env::set_var("TUSE_GLOBAL_CONFIG", cfg.clone());
        }
        Options {
            cwd: dir.clone(),
            mode: Mode::Json,
            resume: None,
            start: Start::Launch,
            flags: Default::default(),
        }
    })
    .unwrap();
    let rx = s.subscribe();

    std::fs::write(&cfg, config("v2").replace("user_message", "process_start")).unwrap();
    s.run_command("test:again", &[]).unwrap();
    wait_tag(&s, "v2");
    assert_eq!(s.config().prompts["other:untouched"], "v2");

    std::fs::write(&cfg, config("v3")).unwrap();
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
    assert_eq!(s.config().prompts["other:untouched"], "v3");

    std::fs::write(&cfg, config("v4")).unwrap();
    s.submit(UserInput::text("hook only"));
    wait_tag(&s, "v4");
    assert_eq!(s.config().prompts["other:untouched"], "v4");

    std::fs::write(&cfg, "return { bad = { typo_key = 1 } }").unwrap();
    s.run_command("test:again", &[]).unwrap();
    let end = Instant::now() + Duration::from_secs(5);
    loop {
        match rx.recv_timeout(end - Instant::now()).expect("an error") {
            Event::Error(e) if e.contains("typo_key") => break,
            _ => {}
        }
    }
    assert_eq!(s.config().prompts["test:tag"], "v4");
    s.close();
    common::cleanup(&dir);
}
