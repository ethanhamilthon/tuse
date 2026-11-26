#[path = "../common/mod.rs"]
mod common;
use common::Reply;
use serde_json::json;
use std::fs;
use std::time::{Duration, Instant};
use tuse_core::{Event, Mode, Options, Record, Session, Start, UserInput};

fn run_lua(s: &std::sync::Arc<Session>, rx: &std::sync::mpsc::Receiver<Event>) -> (String, usize) {
    s.submit(UserInput::text("go"));
    let (mut out, mut diffs) = (String::new(), 0);
    let dl = Instant::now() + Duration::from_secs(15);
    while Instant::now() < dl {
        match rx.recv_timeout(Duration::from_millis(200)) {
            Ok(Event::ToolEnd {
                output, diffs: d, ..
            }) => {
                out = output;
                diffs = d.len();
            }
            Ok(Event::TurnEnd { .. }) => return (out, diffs),
            _ => {}
        }
    }
    panic!("turn did not finish");
}

#[test]
fn agent_only_functions_isolation_overrides_disables_and_reload() {
    let probe1 = r#"
assert(my_secret_fn("hi") == "res:hi")
assert(docs("lua"):find("my_secret_fn", 1, true) == nil)
assert(sh("echo a").out == "wrapped:echo a")
assert(require("tuse").sh("echo b").out == "wrapped:echo b")
assert(write("w.txt", "hello"):find("wrapped:", 1, true) == 1)
assert(require("tuse").my_secret_fn == nil)
assert(pcall(function() require("tuse").sh = 1 end) == false)
assert(pcall(rawset, require("tuse"), "sh", 1) == false)
assert(pcall(setmetatable, require("tuse"), {}) == false)
assert(pcall(rawset, json, "encode", 1) == false)
assert(pcall(setmetatable, json, {}) == false)
assert(getmetatable("") == "string")
assert(getmetatable(sh("echo m")) == nil or type(getmetatable(sh("echo m"))) == "string")
package.loaded.tuse = { sh = function() return "hijacked" end }
assert(require("tuse").sh("echo c").out == "wrapped:echo c")
os.getenv = function() return "agent-env-tampered" end
coroutine.resume = function() error("agent-co-tampered") end
table.sort = function() error("agent-sort-tampered") end
string.format = function() error("agent-fmt-tampered") end
assert(trusted_stdlib_ok() == "ok")
assert(other_fn() == "no-secret")
package.loaded["trusted_mod"] = { val = "tampered" }
assert(read_trusted_mod() == "original")
assert(wrap_cb(function() return tostring(sh == require("tuse").sh) .. ":" .. tostring(type(my_secret_fn)) end) == "true:function")
saved_sh = sh
saved_enc = json.encode
saved_tuse = require("tuse")
saved_ws = my_secret_fn
return { b = 2, a = 1 }
"#;

    let probe_after_failed_reload = r#"
assert(saved_ws("still") == "res:still")
assert(saved_sh("echo still").out == "wrapped:echo still")
assert(trusted_stdlib_ok() == "ok")
return "still-v1"
"#;

    let probe2 = r#"
assert(sh == nil and _G.sh == nil and require("tuse").sh == nil and package.loaded.tuse.sh == nil)
assert(read == nil and json == nil and inspect == nil and docs == nil and task_get == nil)
assert(saved_tuse.sh == nil and saved_tuse.json == nil and saved_tuse.docs == nil)
assert(load("return sh")() == nil)
assert(require("agent_check") == "all-nil")
local ok1, e1 = pcall(saved_sh, "echo x")
local ok2, e2 = pcall(saved_enc, { a = 1 })
local ok3, e3 = pcall(saved_ws, "q")
assert(not ok1 and tostring(e1):find("sh is disabled", 1, true), tostring(e1))
assert(not ok2 and tostring(e2):find("json is disabled", 1, true), tostring(e2))
assert(not ok3 and tostring(e3):find("my_secret_fn is unavailable", 1, true), tostring(e3))
local cb_res = wrap_cb(function() return tostring(sh) .. ":" .. tostring(require("tuse").sh) end)
assert(cb_res == "nil:nil", cb_res)
local t1 = spawn(function() return tostring(sh) .. ":" .. tostring(require("tuse").sh) end, "ag")
local t2 = spawn_via_trusted(function() return tostring(sh) .. ":" .. tostring(require("tuse").sh) end)
local out1, out2 = wait_tasks(t1, t2)
assert(out1:find("nil:nil", 1, true), out1)
assert(out2:find("nil:nil", 1, true), out2)
local dl = check_docs("lua")
local seen = dl:sub(dl:find("What the agent sees", 1, true))
assert(not seen:find("`sh(", 1, true) and not seen:find("`delete(", 1, true) and not seen:find("`docs(", 1, true))
assert(seen:find("`read(", 1, true) == nil)
assert(check_docs("config"):find("Disabled in this session:", 1, true) == 1)
return { a = 1 }
"#;

    let probe3 = r#"
assert(type(sh) == "function" and sh == require("tuse").sh and saved_tuse.sh == sh)
assert(getmetatable(saved_sh("printf restored")) == "sh")
assert(saved_sh("printf restored").out == "restored")
assert(saved_enc({ x = 2 }) == '{"x":2}')
assert(docs("lua"):find("`sh(", 1, true) ~= nil)
assert(docs("lua"):find("`delete(", 1, true) ~= nil)
return "ok3"
"#;

    let addr = common::provider(vec![
        Reply::Call("lua", json!({ "code": probe1 }).to_string()),
        Reply::Text("t1"),
        Reply::Call(
            "lua",
            json!({ "code": probe_after_failed_reload }).to_string(),
        ),
        Reply::Text("t1b"),
        Reply::Call("lua", json!({ "code": probe2 }).to_string()),
        Reply::Text("t2"),
        Reply::Call("lua", json!({ "code": probe3 }).to_string()),
        Reply::Text("t3"),
    ]);
    let (dir, cfg) = common::setup("agent-fns", &addr, "");
    fs::write(dir.join("note.md"), "hi").unwrap();
    fs::write(dir.join("trusted_mod.lua"), "return { val = 'original' }").unwrap();
    fs::write(
        dir.join("agent_check.lua"),
        "return (sh == nil and read == nil and json == nil and inspect == nil) and 'all-nil' or 'leaked'",
    )
    .unwrap();
    let cfg_v1 = r#"
local t = require("tuse")
local tm = require("trusted_mod")
assert(my_secret_fn == nil)
return {
  functions = {
    my_secret_fn = function(q)
      assert(my_secret_fn == nil)
      return "res:" .. q
    end,
    trusted_stdlib_ok = function()
      assert(os.getenv("THIS_PATH") ~= "agent-env-tampered")
      local nums = { 3, 1, 2 }
      table.sort(nums)
      assert(nums[1] == 1 and string.format("%d", 7) == "7")
      local co = coroutine.create(function() return "co-ok" end)
      local ok, res = coroutine.resume(co)
      assert(ok and res == "co-ok")
      return "ok"
    end,
    other_fn = function()
      return my_secret_fn == nil and "no-secret" or "leaked"
    end,
    read_trusted_mod = function()
      return require("trusted_mod").val .. (tm.val == "original" and "" or "bad")
    end,
    wrap_cb = function(cb)
      assert(t.sh("printf ok").out == "ok")
      return cb()
    end,
    sh = function(cmd, opts)
      local r = t.sh("printf wrapped:%s " .. cmd, opts)
      return { out = "wrapped:" .. cmd, ok = r.ok, code = r.code }
    end,
    write = function(path, content)
      return "wrapped:" .. t.write(path, content)
    end,
  },
  commands = {
    check_cmd = { run = function()
      assert(my_secret_fn == nil)
      local out = t.sh("printf trusted-sh").out
      t.show("cmd", out, false)
    end },
  },
  open = {
    md = function(path)
      assert(my_secret_fn == nil)
      t.show("opened", t.sh("printf open-ok").out .. ":" .. path, false)
    end,
  },
  hooks = {
    check_hook = {
      events = { "session_created" },
      run = function()
        assert(my_secret_fn == nil)
        local id = t.spawn(function()
          assert(my_secret_fn == nil)
          return t.sh("printf task-ok").out
        end, "trusted-bg")
        assert(id ~= nil)
      end,
    },
  },
}
"#;
    fs::write(&cfg, cfg_v1).unwrap();

    let s = Session::open(Options {
        cwd: dir.clone(),
        config: Some(cfg.clone()),
        mode: Mode::Json,
        resume: None,
        start: Start::Launch,
    })
    .unwrap();
    let cfg_json = serde_json::to_value(s.config().as_ref()).unwrap();
    assert_eq!(cfg_json["functions"]["my_secret_fn"], "custom");
    assert_eq!(cfg_json["functions"]["sh"], "override");

    let rx = s.subscribe();
    let (out1, diffs1) = run_lua(&s, &rx);
    assert_eq!(out1.trim(), "{a = 1, b = 2}");
    assert_eq!(diffs1, 1);

    s.run_command("check_cmd", &[]).unwrap();
    s.open_path(&dir.join("note.md"));
    let dl = Instant::now() + Duration::from_secs(5);
    while Instant::now() < dl {
        let mds = s.ui_snapshot().markdowns;
        if mds.iter().any(|m| m.text == "trusted-sh")
            && mds
                .iter()
                .any(|m| m.name == "opened" && m.text.starts_with("open-ok:"))
        {
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    let mds = s.ui_snapshot().markdowns;
    assert!(mds.iter().any(|m| m.text == "trusted-sh"));
    assert!(
        mds.iter()
            .any(|m| m.name == "opened" && m.text.starts_with("open-ok:"))
    );

    fs::write(&cfg, "return { functions = { typo = false } }").unwrap();
    let err = s.reload().unwrap_err();
    assert!(
        err.contains("functions.typo: cannot disable unknown function"),
        "{err}"
    );

    let (out1b, _) = run_lua(&s, &rx);
    assert_eq!(out1b.trim(), "still-v1");

    let cfg_v2 = r#"
local t = require("tuse")
return {
  functions = {
    sh = false,
    read = false,
    delete = false,
    json = false,
    inspect = false,
    docs = false,
    task_get = false,
    wrap_cb = function(cb)
      assert(t.sh("printf still-trusted").out == "still-trusted")
      return cb()
    end,
    spawn_via_trusted = function(cb)
      return t.spawn(cb, "wrapped-agent-cb")
    end,
    wait_tasks = function(a, b)
      for _ = 1, 50 do
        if t.task_get(a).status ~= "running" and t.task_get(b).status ~= "running" then break end
        t.sh("sleep 0.02")
      end
      return t.task_get(a).output, t.task_get(b).output
    end,
    check_docs = function(topic)
      return t.docs(topic)
    end,
  },
}
"#;
    fs::write(&cfg, cfg_v2).unwrap();
    s.reload().unwrap();
    let cfg_json2 = serde_json::to_value(s.config().as_ref()).unwrap();
    assert_eq!(cfg_json2["functions"]["sh"], "disabled");

    let (out2, _) = run_lua(&s, &rx);
    assert_eq!(out2.trim(), "{a = 1}");

    let notifs: Vec<String> = s
        .records()
        .into_iter()
        .filter_map(|r| match r {
            Record::Notification { text } => Some(text),
            _ => None,
        })
        .collect();
    assert!(
        notifs
            .iter()
            .any(|n| n.contains("(ag)") && !n.contains("task_get")),
        "{notifs:?}"
    );

    fs::write(&cfg, "return {}").unwrap();
    s.reload().unwrap();
    let (out3, _) = run_lua(&s, &rx);
    assert_eq!(out3.trim(), "ok3");

    assert!(
        s.records()
            .iter()
            .any(|r| matches!(r, Record::Lua { output, .. } if output.trim() == "{a = 1}"))
    );
    s.close();
    common::cleanup(&dir);
}
