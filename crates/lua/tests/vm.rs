use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tuse_lua::{Done, Host, Stop, TaskStart, Vm};

#[derive(Default)]
struct H {
    ended: Mutex<Vec<(String, String)>>,
}

impl Host for H {
    fn cwd(&self) -> PathBuf {
        std::env::temp_dir()
    }
    fn task_start(
        &self,
        name: &str,
        _code: &str,
        _from_lua_hook: bool,
    ) -> Result<TaskStart, String> {
        let log = std::env::temp_dir().join(format!("tuse-lua-{}-{name}.log", std::process::id()));
        let log = std::fs::File::create(log).map_err(|e| e.to_string())?;
        Ok(TaskStart {
            id: name.into(),
            log,
            cancel: Arc::default(),
        })
    }
    fn task_end(&self, id: &str, d: Done) {
        self.ended.lock().unwrap().push((id.into(), d.text));
    }
}

fn stop(secs: u64) -> Stop {
    Stop::new(
        Arc::new(AtomicBool::new(false)),
        Some(Instant::now() + Duration::from_secs(secs)),
    )
}

#[test]
fn runs_code_keeps_globals_and_shows_values() {
    let vm = Vm::new(Arc::new(H::default()), |_| {});
    let d = vm.run("x = 20\nprint('hi', 1)", "lua", stop(5));
    assert!(d.ok && d.text == "hi\t1\n", "{}", d.text);
    let d = vm.run("x * 2 + 2", "lua", stop(5));
    assert_eq!(d.text, "42");
    let d = vm.run("{a = 1, 'x', b = {true}}", "lua", stop(5));
    assert_eq!(d.text, "{\"x\", a = 1, b = {true}}");
    let d = vm.run("local t = nil\nreturn t.y", "lua", stop(5));
    assert!(
        !d.ok && d.text.contains("lua:2:") && d.text.contains("traceback"),
        "{}",
        d.text
    );
    let d = vm.run(
        "sh('echo $MY_X; exit 3', {env = {MY_X = '42'}})",
        "lua",
        stop(5),
    );
    assert_eq!(d.text, "42\n[exit code 3]");
    let d = vm.run("json.encode(json.decode('{\"a\":[1,2]}'))", "lua", stop(5));
    assert_eq!(d.text, "{\"a\":[1,2]}");
}

#[test]
fn deadlines_stop_loops_and_commands_but_not_others() {
    let h = Arc::new(H::default());
    let vm = Vm::new(h.clone(), |_| {});
    let started = Instant::now();
    let d = vm.run("while true do end", "lua", stop(1));
    assert!(!d.ok && d.stop == Some("timed out"), "{}", d.text);
    let d = vm.run("print('a'); sh('sleep 30')", "lua", stop(1));
    assert!(!d.ok && d.text.starts_with("a\n"), "{}", d.text);
    assert!(started.elapsed() < Duration::from_secs(5));
    let d = vm.run(
        "n = 0\nspawn(function() for i = 1, 3 do n = n + 1; sh('sleep 0.05') end print('done', n) end, 't')\nsh('sleep 0.5')\nreturn n",
        "lua",
        stop(5),
    );
    assert_eq!(d.text, "3");
    std::thread::sleep(Duration::from_millis(100));
    assert_eq!(h.ended.lock().unwrap().len(), 1);
    let d = vm.run(
        "local co = coroutine.wrap(function() local r = sh('echo in'); coroutine.yield(r.out); return 'end' end)\nreturn co(), co()",
        "lua",
        stop(5),
    );
    assert_eq!(
        d.text,
        "\"in\\n\"\tend"
            .replace("\\n", "\n")
            .replace("\"in\n\"", "in\n"),
        "{}",
        d.text
    );
}

#[test]
fn sh_has_no_controlling_terminal() {
    let vm = Vm::new(Arc::new(H::default()), |_| {});
    let d = vm.run(
        "sh('(exec 3<>/dev/tty) 2>/dev/null && echo tty || echo none').out",
        "lua",
        stop(5),
    );
    assert_eq!(d.text, "none\n");
}

#[test]
fn external_wait_pauses_and_resumes_timeout_budget() {
    let (tx, rx) = std::sync::mpsc::channel();
    let rx = Arc::new(Mutex::new(Some(rx)));
    let vm = Vm::new(Arc::new(H::default()), move |lua| {
        let rx = rx.clone();
        let start = lua
            .create_function(move |lua, ()| {
                let rx = rx.lock().unwrap().take().unwrap();
                let key = tuse_lua::add_wait(
                    lua,
                    tuse_lua::ExtWait {
                        rx,
                        pause_timeout: true,
                        on_cancel: None,
                    },
                )?;
                Ok(key)
            })
            .unwrap();
        let f = tuse_lua::yield_wait(lua, start).unwrap();
        lua.globals().set("pause_wait", f).unwrap();
    });
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(500));
        let _ = tx.send(Ok(serde_json::json!("ok")));
    });
    let stop = Stop::new(
        Arc::default(),
        Some(Instant::now() + Duration::from_millis(300)),
    );
    let d = vm.run(
        "local v = pause_wait(); sh('sleep 0.05'); return v",
        "lua",
        stop,
    );
    assert!(d.ok && d.text == "ok", "ok={} text={}", d.ok, d.text);
}
