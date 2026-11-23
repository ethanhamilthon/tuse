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
    panic!(
        "the hook wrote fewer than {n} lines: {:?}",
        std::fs::read_to_string(path).unwrap_or_default()
    );
}

#[test]
fn lua_hooks_cover_tool_command_task_open_without_recursion() {
    let _env = ENV.lock().unwrap_or_else(|e| e.into_inner());
    let code = "spawn(function() sleep(0.02); return 'bg-ok' end, 'bg'); return 'tool-ok'";
    let addr = common::provider(vec![
        Reply::Call("lua", json!({"code": code}).to_string()),
        Reply::Text("done"),
    ]);
    let (dir, _) = common::setup("hook-lua", &addr, "");
    let note = dir.join("demo.md");
    std::fs::write(&note, "# hi").unwrap();
    let log = dir.join("lua_events");
    let cfg = dir.join("main.lua");
    std::fs::write(
        &cfg,
        format!(
            r#"local t = require("tuse")
local function log_line(s)
  local f = assert(io.open("{}", "a"))
  f:write(s, "\n")
  f:close()
end
return {{
  commands = {{
    boom = function() error("cmd fail") end,
  }},
  open = {{
    md = function(path) return "opened:" .. path end,
  }},
  hooks = {{
    watch = {{
      events = {{"lua_start", "lua_end"}},
      run = function(e)
        if e.event == "lua_start" then
          -- Spawning inside a lua_start hook must NOT recursively trigger lua_start/lua_end.
          if e.kind == "open" then
            t.spawn(function() end, "from_hook")
          end
          log_line(table.concat({{
            "start", e.id, e.kind, tostring(e.name or ""), tostring(e.code ~= nil)
          }}, "|"))
        else
          assert(type(e.duration_ms) == "number")
          log_line(table.concat({{
            "end", e.id, e.kind, e.status, tostring(e.result or e.error or "")
          }}, "|"))
        end
      end,
    }},
  }},
}}
"#,
            log.display()
        ),
    )
    .unwrap();
    let s = Session::open(Options {
        cwd: dir.clone(),
        config: Some(cfg),
        mode: Mode::Json,
        resume: None,
        start: Start::Launch,
    })
    .unwrap();
    let events = s.subscribe();
    s.submit(UserInput::text("run"));
    wait_turn(&events);
    s.run_command("boom").unwrap();
    s.open_path(&note);
    let lines = wait_lines(&log, 8);
    std::thread::sleep(Duration::from_millis(150));
    s.close();
    let all = std::fs::read_to_string(&log).unwrap();
    assert!(
        !all.contains("from_hook"),
        "hook spawned task must not recurse: {all}"
    );
    assert!(
        lines
            .iter()
            .any(|l| l.starts_with("start|") && l.contains("|tool||true")),
        "{all}"
    );
    assert!(
        lines
            .iter()
            .any(|l| l.starts_with("end|") && l.contains("|tool|success|tool-ok")),
        "{all}"
    );
    assert!(
        lines
            .iter()
            .any(|l| l.starts_with("start|") && l.contains("|task|bg|false")),
        "{all}"
    );
    assert!(
        lines
            .iter()
            .any(|l| l.starts_with("end|") && l.contains("|task|success|")),
        "{all}"
    );
    assert!(
        lines
            .iter()
            .any(|l| l.starts_with("start|") && l.contains("|command|boom|false")),
        "{all}"
    );
    assert!(
        lines.iter().any(|l| l.starts_with("end|")
            && l.contains("|command|error|")
            && l.contains("cmd fail")),
        "{all}"
    );
    assert!(
        lines
            .iter()
            .any(|l| l.starts_with("start|") && l.contains("|open|md|false")),
        "{all}"
    );
    assert!(
        lines
            .iter()
            .any(|l| l.starts_with("end|") && l.contains("|open|success|opened:")),
        "{all}"
    );
    common::cleanup(&dir);
}
