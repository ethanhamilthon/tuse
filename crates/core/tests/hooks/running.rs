use super::*;

fn wait_lines(path: &std::path::Path, n: usize) -> Vec<String> {
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(30) {
        let text = std::fs::read_to_string(path).unwrap_or_default();
        if text.lines().count() >= n {
            return text.lines().map(String::from).collect();
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    panic!("the hook wrote fewer than {n} lines");
}

#[test]
fn agent_message_lists_only_running_tasks_and_children() {
    let _env = ENV.lock().unwrap_or_else(|e| e.into_inner());
    let code = "spawn(function() while true do sleep(0.1) end end, 'loop')";
    let addr = common::provider(vec![
        Reply::Text("one"),
        Reply::Call("lua", json!({"code": code}).to_string()),
        Reply::Text("two"),
        Reply::Stall("partial"),
    ]);
    let (dir, _) = common::setup("hook-running", &addr, "");
    let log = dir.join("events");
    let cfg = dir.join("main.lua");
    std::fs::write(
        &cfg,
        format!(
            r#"local function names(v, key)
  if v == nil then return "nil" end
  local out = {{}}
  for _, x in ipairs(v) do out[#out + 1] = x[key] end
  return "[" .. table.concat(out, ",") .. "]"
end
return {{
  model = "p/m",
  hooks = {{
    tui_only = {{
      modes = {{"tui"}},
      events = {{"agent_message"}},
      run = function(e) io.open("{unexpected}", "a"):write("x"):close() end,
    }},
    record = {{
      modes = {{"json"}},
      events = {{"user_message", "agent_message"}},
      run = function(e)
        local f = io.open("{}", "a")
        f:write(e.event, " ", tostring(e.status), " ", names(e.tasks, "name"), " ", names(e.children, "id"), "\n")
        f:close()
      end,
    }},
  }},
}}
"#,
            log.display(),
            unexpected = dir.join("unexpected").display()
        ),
    )
    .unwrap();
    let open = || {
        Session::open(Options {
            cwd: dir.clone(),
            config: Some(cfg.clone()),
            mode: Mode::Json,
            resume: None,
            start: Start::Launch,
        })
        .unwrap()
    };
    let s = open();
    let events = s.subscribe();
    s.submit(UserInput::text("a"));
    wait_turn(&events);
    let lines = wait_lines(&log, 2);
    assert_eq!(lines[0], "user_message nil nil nil");
    assert_eq!(lines[1], "agent_message finished [] []");

    // A child with an explicit parent is found before its first prompt.
    // SAFETY: the tests of this file take turns on `ENV`.
    unsafe { std::env::set_var("TUSE_PARENT", &s.id) };
    let child = open();
    unsafe { std::env::remove_var("TUSE_PARENT") };
    let found = tuse_core::descendants(&s.id, &s.config());
    assert_eq!(found.len(), 1);
    assert!(found[0].running && found[0].id == child.id);

    s.submit(UserInput::text("b"));
    wait_turn(&events);
    let lines = wait_lines(&log, 4);
    assert_eq!(
        lines[3],
        format!("agent_message finished [loop] [{}]", child.id)
    );

    s.submit(UserInput::text("c"));
    wait_text(&events);
    std::thread::sleep(Duration::from_millis(200));
    s.cancel();
    let lines = wait_lines(&log, 6);
    assert_eq!(
        lines[5],
        format!("agent_message cancelled [loop] [{}]", child.id)
    );

    child.close();
    s.close();
    assert!(!dir.join("unexpected").exists());
    std::fs::remove_dir_all(dir).ok();
}
