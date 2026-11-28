#[path = "../../core/tests/common/mod.rs"]
mod common;
use common::Reply;
use serde_json::json;
use tuse_core::{Mode, Options, Session, Start};

mod support;
use support::{call, event, spawn};

#[test]
fn a_lua_fork_sends_session_switched() {
    let addr = common::provider(vec![Reply::Text("an answer")]);
    let extra = r#"
  commands = {
    back = function()
      local t = require("tuse")
      local h = t.session.get().history
      for i = #h, 1, -1 do
        if h[i].role == "agent" then t.session.fork(i) return end
      end
    end,
  },
"#;
    let (dir, cfg) = common::setup("rpc-lua-switch", &addr, extra);
    let session = Session::open({
        unsafe {
            std::env::set_var("TUSE_GLOBAL_CONFIG", cfg);
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
    let id = session.id.clone();
    let (tx, out, server) = spawn(session);
    call(&tx, &out, 1, "initialize", json!({"protocolVersion": 2}));
    call(&tx, &out, 2, "turn.submit", json!({"text": "hi"}));
    event(&out, "turn_end");
    call(&tx, &out, 3, "commands.run", json!({"name": "test:back"}));
    let switched = event(&out, "session_switched");
    assert_ne!(switched["session"]["id"], id.as_str(), "{switched}");
    assert_eq!(switched["draft"], "");
    let info = call(&tx, &out, 4, "session.state", json!({}));
    assert_eq!(info["result"]["id"], switched["session"]["id"], "{info}");
    call(&tx, &out, 5, "shutdown", json!({}));
    server.join().unwrap();
    common::cleanup(&dir);
}
