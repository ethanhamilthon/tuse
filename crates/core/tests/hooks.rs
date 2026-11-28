mod common;
use common::Reply;
use std::sync::Mutex;
use std::time::Duration;

use tuse_core::{Event, Mode, Options, Session, Start, UserInput};

static ENV: Mutex<()> = Mutex::new(());

#[test]
fn lifecycle_order_and_stopped_reply() {
    let _env = ENV.lock().unwrap_or_else(|e| e.into_inner());
    let addr = common::provider(vec![Reply::Stall("partial")]);
    let (dir, cfg) = common::setup(
        "hooks",
        &addr,
        r#"
        hook = (function()
            local function run(e)
                local t = require('tuse')
                local s = t.session.get()
                assert(s.history[1].role == 'system')
                local n = 0
                for _ in pairs(e) do n = n + 1 end
                assert(n == (e.event == 'agent_message' and 2 or 1))
                if e.event == 'process_start' then marker = true end
                if e.event == 'session_resumed' then assert(marker == nil) end
                local f = assert(io.open(s.cwd .. '/events', 'a'))
                f:write(e.event, '\n'); f:close()
            end
            return {
                config_load = run, process_start = run, session_created = run,
                session_resumed = run, session_close = run, process_end = run,
                user_message = run, turn_stopped = run, agent_message = run,
            }
        end)(),
    "#,
    );
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
    let fresh = s.switch(None).unwrap();
    let rx = fresh.subscribe();
    fresh.submit(UserInput::text("go"));
    loop {
        if matches!(
            rx.recv_timeout(Duration::from_secs(5)).unwrap(),
            Event::Text(_)
        ) {
            break;
        }
    }
    fresh.cancel();
    assert!(fresh.history().iter().any(|r| r.role == "agent"
        && r.fields["cancelled"] == true
        && r.fields["text"] == "partial"));
    let resumed = fresh.switch(Some(s.id.clone())).unwrap();
    resumed.close();
    let log = std::fs::read_to_string(dir.join("events")).unwrap();
    let life: Vec<_> = log
        .lines()
        .filter(|e| !matches!(*e, "user_message" | "agent_message" | "turn_stopped"))
        .collect();
    assert_eq!(
        life,
        [
            "config_load",
            "process_start",
            "session_created",
            "session_close",
            "config_load",
            "session_created",
            "session_close",
            "config_load",
            "session_resumed",
            "process_end"
        ]
    );
    assert!(log.contains("user_message\nagent_message\nturn_stopped\n"));
    common::cleanup(&dir);
}
