use serde_json::json;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex, Weak};
use std::time::{Duration, Instant};
use tuse_lua::mlua::{self, Lua, Table, UserData, UserDataMethods, UserDataRef, Value};
use tuse_lua::{ExtWait, add_wait};

enum ProcEvent {
    Line(String),
    Exit(Option<i32>, bool),
}

pub(crate) struct ProcShared {
    pid: u32,
    killed: AtomicBool,
    rx: Mutex<Receiver<ProcEvent>>,
}

impl ProcShared {
    pub(crate) fn kill(&self) {
        if !self.killed.swap(true, Ordering::SeqCst) {
            tuse_lua::kill_group(self.pid);
        }
    }
}

impl Drop for ProcShared {
    fn drop(&mut self) {
        self.kill();
    }
}

pub(crate) struct ProcHandle(pub(crate) Arc<ProcShared>);

impl UserData for ProcHandle {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("kill", |_, this, ()| {
            this.0.kill();
            Ok(())
        });
    }
}

pub(crate) fn spawn(
    lua: &Lua,
    argv_t: Table,
    opts: Option<Table>,
    active: &mut Vec<Weak<ProcShared>>,
) -> mlua::Result<mlua::AnyUserData> {
    let argv: Vec<String> = argv_t
        .sequence_values::<String>()
        .collect::<mlua::Result<_>>()?;
    if argv.is_empty() {
        return Err(mlua::Error::runtime("argv must not be empty"));
    }
    let timeout = opts
        .as_ref()
        .and_then(|o| o.get::<Option<f64>>("timeout").ok().flatten());
    let stdin_data = opts
        .as_ref()
        .and_then(|o| o.get::<Option<String>>("stdin").ok().flatten());
    let (reader, writer) = std::io::pipe().map_err(|e| mlua::Error::runtime(e.to_string()))?;
    let mut cmd = Command::new(&argv[0]);
    let w2 = writer
        .try_clone()
        .map_err(|e| mlua::Error::runtime(e.to_string()))?;
    cmd.args(&argv[1..])
        .stdin(if stdin_data.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(w2)
        .stderr(writer);
    unsafe { cmd.pre_exec(tuse_lua::setsid) };
    let mut child = cmd
        .spawn()
        .map_err(|e| mlua::Error::runtime(format!("{}: {e}", argv[0])))?;
    drop(cmd);
    if let Some(data) = stdin_data
        && let Some(mut sin) = child.stdin.take()
    {
        std::thread::spawn(move || {
            let _ = sin.write_all(data.as_bytes());
        });
    }
    let (tx, rx) = mpsc::channel();
    let shared = Arc::new(ProcShared {
        pid: child.id(),
        killed: AtomicBool::new(false),
        rx: Mutex::new(rx),
    });
    active.retain(|w| w.strong_count() > 0);
    active.push(Arc::downgrade(&shared));
    let timed_out = Arc::new(AtomicBool::new(false));
    let done = Arc::new(AtomicBool::new(false));
    if let Some(secs) = timeout {
        let (s2, to2, d2) = (shared.clone(), timed_out.clone(), done.clone());
        let dl = Instant::now() + Duration::from_secs_f64(secs.max(0.0));
        std::thread::spawn(move || {
            while !d2.load(Ordering::Acquire) && !s2.killed.load(Ordering::Acquire) {
                if Instant::now() >= dl {
                    to2.store(true, Ordering::Release);
                    s2.kill();
                    break;
                }
                std::thread::sleep(Duration::from_millis(50));
            }
        });
    }
    let (s3, to3, d3) = (shared.clone(), timed_out, done);
    std::thread::spawn(move || {
        for line in BufReader::new(reader).lines().map_while(Result::ok) {
            if tx.send(ProcEvent::Line(line)).is_err() {
                break;
            }
        }
        let code = child.wait().ok().and_then(|s| s.code());
        s3.killed.store(true, Ordering::SeqCst);
        d3.store(true, Ordering::Release);
        let _ = tx.send(ProcEvent::Exit(code, to3.load(Ordering::Acquire)));
    });
    lua.create_userdata(ProcHandle(shared))
}

pub(crate) fn next(lua: &Lua, handle: UserDataRef<ProcHandle>) -> mlua::Result<Value> {
    let (shared, cancel_shared) = (handle.0.clone(), handle.0.clone());
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let ev = match shared.rx.lock().unwrap().recv() {
            Ok(ProcEvent::Line(line)) => json!({"exited": false, "line": line}),
            Ok(ProcEvent::Exit(code, timed_out)) => {
                json!({"exited": true, "code": code, "timed_out": timed_out})
            }
            Err(_) => json!({"exited": true, "code": null, "timed_out": false}),
        };
        let _ = tx.send(Ok(ev));
    });
    let wait = ExtWait {
        rx,
        pause_timeout: false,
        on_cancel: Some(Box::new(move || cancel_shared.kill())),
    };
    Ok(Value::Integer(add_wait(lua, wait)? as i64))
}
