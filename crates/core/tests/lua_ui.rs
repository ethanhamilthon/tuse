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
            serde_json::json!({"code": "ran = 0; spawn(function() sleep(0.05); ran = 1 end, 'bg'); local p = show_picker({'yes'}); return tostring(p) .. ':' .. tostring(ran)"}).to_string(),
        ),
        common::Reply::Text("ok"),
    ]);
    let extra = r###"
  commands = {
    ask_flow = function()
      local t = require("tuse")
      t.show_markdown("note", "## First", false)
      t.show_markdown("note", "## Updated", false)
      t.show_markdown("pin1", "Pinned A", true)
      t.show_markdown("gone", "Temp", false)
      t.show_markdown("gone", "", false)
      local pick = t.show_picker({"alpha", "beta"})
      local ans = t.show_question("**Name?**")
      local skip = t.show_picker({"x", "y"})
      t.show_markdown("summary", tostring(pick) .. "|" .. tostring(ans) .. "|" .. tostring(skip), true)
    end,
    second = function()
      local t = require("tuse")
      local v = t.show_question("Queued?")
      t.show_markdown("q2", tostring(v), false)
    end,
  },
  hooks = {
    interactive = {
      events = {"process_start", "process_end"},
      run = function(e)
        if e.event == "process_start" and e.reason == "reload" then return end
        require("tuse").show_question("hook " .. e.event)
      end,
    },
  },
"###;
    let (dir, cfg) = common::setup("lua-ui", &addr, extra);
    let s = Session::open(Options {
        cwd: dir.clone(),
        config: Some(cfg.clone()),
        mode: Mode::Tui,
        resume: None,
        start: Start::Launch,
    })
    .unwrap();
    let rx = s.subscribe();
    // Interactive process_start hook is waiting on a question: reload must NOT deadlock!
    let hreq = next_req(&rx);
    assert!(matches!(hreq.prompt, UiPrompt::Question { .. }));
    let t0 = Instant::now();
    s.reload().unwrap();
    assert!(t0.elapsed() < Duration::from_secs(3), "reload deadlocked");

    // Enqueue two custom commands back-to-back: FIFO ordering, one active at a time.
    s.run_command("ask_flow").unwrap();
    let r1 = next_req(&rx);
    assert_eq!(
        r1.prompt,
        UiPrompt::Picker {
            options: vec!["alpha".into(), "beta".into()]
        }
    );
    s.run_command("second").unwrap();
    std::thread::sleep(Duration::from_millis(50));
    assert_eq!(s.ui_snapshot().active.as_ref().map(|r| r.id), Some(r1.id));

    // Answer picker, question (from second is queued behind ask_flow's picker, then ask_flow's question).
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
    s.ui_reply(r4.id, None).unwrap(); // cancelled -> nil, no UI entry

    // Wait for final summary markdown update.
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
        ("picker", "Picker", "beta")
    );
    assert_eq!(
        (
            snap.entries[2].kind,
            snap.entries[2].prompt.as_str(),
            snap.entries[2].value.as_str()
        ),
        ("question", "**Name?**", "alice")
    );

    // Reload keeps markdown blocks (with pin state) and UI entries, updates custom commands.
    std::fs::write(
        &cfg,
        "return { model = 'p/m', commands = { updated_cmd = function() require('tuse').show_question('still open') end } }",
    )
    .unwrap();
    s.reload().unwrap();
    assert_eq!(s.config().commands, ["updated_cmd"]);
    let after_reload = s.ui_snapshot();
    assert_eq!(after_reload.markdowns, snap.markdowns);
    assert_eq!(after_reload.entries, snap.entries);

    // Built-in commands cannot be overridden.
    std::fs::write(
        &cfg,
        "return { model = 'p/m', commands = { reload = function() end } }",
    )
    .unwrap();
    assert!(s.reload().unwrap_err().contains("cannot override built-in"));

    // Tool call calling show_picker yields so other Lua jobs keep running, then finishes on reply.
    s.submit(UserInput::text("persist me"));
    let treq = next_req(&rx);
    std::thread::sleep(Duration::from_millis(120));
    s.ui_reply(treq.id, Some("yes".into())).unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    while s.busy() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(
        s.records().iter().any(
            |r| matches!(r, tuse_core::Record::Tool { output, .. } if output.contains("yes:1"))
        )
    );
    s.run_command("updated_cmd").unwrap();
    let pending = next_req(&rx);
    assert_eq!(
        s.ui_snapshot().active.as_ref().map(|r| r.id),
        Some(pending.id)
    );
    let id = s.id.clone();
    s.close();
    assert!(s.ui_snapshot().active.is_none());

    std::fs::write(&cfg, "return { model = 'p/m' }").unwrap();
    let resumed = Session::open(Options {
        cwd: dir.clone(),
        config: Some(cfg),
        mode: Mode::Tui,
        resume: Some(id),
        start: Start::Resume,
    })
    .unwrap();
    let rsnap = resumed.ui_snapshot();
    assert!(rsnap.markdowns.is_empty() && rsnap.entries.is_empty() && rsnap.active.is_none());
    resumed.close();
    common::cleanup(&dir);
}
