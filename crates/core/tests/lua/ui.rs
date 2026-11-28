#[path = "../common/mod.rs"]
mod common;
use std::time::{Duration, Instant};
use tuse_core::{Event, Mode, Options, Session, Start, UiPrompt, UiRequest, UserInput};

fn next_req(rx: &std::sync::mpsc::Receiver<Event>) -> UiRequest {
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(5) {
        if let Ok(Event::UiRequest(req)) = rx.recv_timeout(Duration::from_millis(50)) {
            return req;
        }
    }
    panic!("timed out waiting for UiRequest");
}

#[test]
fn lua_ui_queue_reload_close_and_custom_commands() {
    let addr = common::provider(vec![
        common::Reply::Call(
            "lua",
            serde_json::json!({"code": "ran = 0; tasks.spawn(function() sh('sleep 0.05'); ran = 1 end, 'bg'); local p = ui.pick({'yes'}); return tostring(p) .. ':' .. tostring(ran)"}).to_string(),
        ),
        common::Reply::Text("ok"),
    ]);
    let extra = r###"
  commands = {
    ask_flow = function()
      local t = require("tuse")
      t.ui.show("note", "## First", false)
      t.ui.show("note", "## Updated", false)
      t.ui.show("pin1", "Pinned A", true)
      t.ui.show("gone", "Temp", false)
      t.ui.show("gone", "", false)
      assert(t.ui.pick({}, "No choices") == nil)
      local pick = t.ui.pick({"alpha", "beta"}, "**Choose one**")
      local ans = t.ui.ask("**Name?**")
      local skip = t.ui.pick({"x", "y"}, "Cancel this?")
      t.ui.show("summary", tostring(pick) .. "|" .. tostring(ans) .. "|" .. tostring(skip), true)
    end,
    second = function()
      local t = require("tuse")
      local v = t.ui.ask("Queued?")
      t.ui.show("q2", tostring(v), false)
    end,
  },
  hook = {
    ui_prompt = function(e)
      assert(e.event == "ui_prompt" and type(e.active) == "boolean")
      local s = require("tuse").session.get()
      local f = assert(io.open(s.cwd .. "/prompt-events", "a"))
      f:write(tostring(e.active), "\n")
      f:close()
    end,
    process_start = function(e)
      require("tuse").ui.ask("hook " .. e.event)
    end,
    process_end = function(e)
      require("tuse").ui.ask("hook " .. e.event)
    end,
  },
"###;
    let (dir, cfg) = common::setup("lua-ui", &addr, extra);
    let s = Session::open({
        unsafe {
            std::env::set_var("TUSE_GLOBAL_CONFIG", cfg.clone());
        }
        Options {
            cwd: dir.clone(),
            mode: Mode::Tui,
            resume: None,
            start: Start::Launch,
            flags: Default::default(),
        }
    })
    .unwrap();
    let rx = s.subscribe();
    let hreq = next_req(&rx);
    assert!(matches!(hreq.prompt, UiPrompt::Question { .. }));
    let t0 = Instant::now();
    s.reload().unwrap();
    assert!(t0.elapsed() < Duration::from_secs(3), "reload deadlocked");

    s.run_command("test:ask_flow", &[]).unwrap();
    let r1 = next_req(&rx);
    assert_eq!(
        r1.prompt,
        UiPrompt::Picker {
            options: vec!["alpha".into(), "beta".into()],
            title: Some("**Choose one**".into()),
        }
    );
    s.run_command("test:second", &[]).unwrap();
    std::thread::sleep(Duration::from_millis(50));
    assert_eq!(s.ui_snapshot().active.as_ref().map(|r| r.id), Some(r1.id));

    s.ui_reply(r1.id, Some("beta".into())).unwrap();
    let r2 = next_req(&rx);
    assert_eq!(
        r2.prompt,
        UiPrompt::Question {
            text: "Queued?".into()
        }
    );
    s.ui_reply(r2.id, Some("yes".into())).unwrap();

    let r3 = next_req(&rx);
    assert_eq!(
        r3.prompt,
        UiPrompt::Question {
            text: "**Name?**".into()
        }
    );
    s.ui_reply(r3.id, Some("alice".into())).unwrap();

    let r4 = next_req(&rx);
    s.ui_reply(r4.id, None).unwrap();

    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if s.ui_snapshot()
            .markdowns
            .iter()
            .any(|m| m.name == "summary" && m.text == "beta|alice|nil")
        {
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    let snap = s.ui_snapshot();
    assert!(snap.markdowns.iter().all(|m| m.name != "gone"));
    assert_eq!(
        snap.markdowns.iter().filter(|m| m.name == "note").count(),
        1
    );
    assert!(snap.markdowns.iter().any(|m| m.name == "pin1" && m.pin));
    assert!(
        snap.markdowns
            .iter()
            .any(|m| m.name == "summary" && m.pin && m.text == "beta|alice|nil")
    );
    assert_eq!(snap.entries.len(), 3, "cancelled picker leaves no entry");
    assert_eq!(
        (
            snap.entries[0].kind,
            snap.entries[0].prompt.as_str(),
            snap.entries[0].value.as_str()
        ),
        ("picker", "**Choose one**", "beta")
    );
    assert_eq!(
        (
            snap.entries[2].kind,
            snap.entries[2].prompt.as_str(),
            snap.entries[2].value.as_str()
        ),
        ("question", "**Name?**", "alice")
    );

    std::fs::write(
        &cfg,
        "return { test = { commands = { updated_cmd = function() require('tuse').ui.ask('still open') end } } }",
    )
    .unwrap();
    s.reload().unwrap();
    let prompt_events = std::fs::read_to_string(dir.join("prompt-events")).unwrap();
    // The first command can queue its next question before or after the
    // second command is answered, but the already queued picker handoff
    // must not emit a false/true pair.
    assert!(
        prompt_events == "true\nfalse\ntrue\nfalse\ntrue\nfalse\n"
            || prompt_events == "true\nfalse\ntrue\nfalse\ntrue\nfalse\ntrue\nfalse\n",
        "{prompt_events:?}"
    );
    assert_eq!(
        s.config().commands.iter().collect::<Vec<_>>(),
        ["test:updated_cmd"]
    );
    let after_reload = s.ui_snapshot();
    assert_eq!(after_reload.markdowns, snap.markdowns);
    assert_eq!(after_reload.entries, snap.entries);

    std::fs::write(
        &cfg,
        "return { test = { commands = { reload = { run = function() end } } } }",
    )
    .unwrap();
    assert!(s.reload().unwrap_err().contains("expected a function"));

    s.submit(UserInput::text("persist me"));
    let treq = next_req(&rx);
    std::thread::sleep(Duration::from_millis(120));
    s.ui_reply(treq.id, Some("yes".into())).unwrap();
    let answered = || {
        s.records()
            .iter()
            .any(|r| matches!(r, tuse_core::Record::Lua { output, .. } if output.contains("yes:1")))
    };
    let deadline = Instant::now() + Duration::from_secs(5);
    while !(answered() && !s.busy()) && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(answered());
    s.run_command("test:updated_cmd", &[]).unwrap();
    let pending = next_req(&rx);
    assert_eq!(
        s.ui_snapshot().active.as_ref().map(|r| r.id),
        Some(pending.id)
    );
    std::fs::write(
        &cfg,
        r#"return { test = { hook = { ui_prompt = function(e)
            local s = require('tuse').session.get()
            local f = assert(io.open(s.cwd .. '/replayed-prompt', 'a'))
            f:write(tostring(e.active), '\n'); f:close()
        end } } }"#,
    )
    .unwrap();
    s.reload().unwrap();
    let id = s.id.clone();
    s.close();
    assert!(s.ui_snapshot().active.is_none());
    assert_eq!(
        std::fs::read_to_string(dir.join("replayed-prompt")).unwrap(),
        "true\nfalse\n"
    );

    std::fs::write(&cfg, "return {}").unwrap();
    let resumed = Session::open({
        unsafe {
            std::env::set_var("TUSE_GLOBAL_CONFIG", cfg);
        }
        Options {
            cwd: dir.clone(),
            mode: Mode::Tui,
            resume: Some(id),
            start: Start::Resume,
            flags: Default::default(),
        }
    })
    .unwrap();
    let rsnap = resumed.ui_snapshot();
    assert_eq!(rsnap.markdowns, snap.markdowns);
    assert_eq!(rsnap.entries.len(), 4);
    assert_eq!(rsnap.entries[..3], snap.entries[..]);
    assert_eq!(
        (
            rsnap.entries[3].kind,
            rsnap.entries[3].prompt.as_str(),
            rsnap.entries[3].value.as_str()
        ),
        ("picker", "Picker", "yes")
    );
    assert!(rsnap.active.is_none());
    resumed.close();
    common::cleanup(&dir);
}
