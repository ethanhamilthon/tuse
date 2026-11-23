use crate::event::Event;
use crate::session::Session;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;
use tuse_lua::mlua::{self, Lua, MultiValue, Value};
use tuse_lua::{Done, Job, Out, Stop};

pub fn openable_url(url: &str) -> bool {
    let u = url.to_ascii_lowercase();
    url.len() < 2000
        && ["http://", "https://", "mailto:"]
            .iter()
            .any(|s| u.starts_with(s))
}

impl Session {
    pub fn open_url(self: &Arc<Self>, url: &str) {
        if !openable_url(url) {
            self.emit(Event::Error(format!("cannot open URL `{url}`")));
            return;
        }
        self.spawn_os_open(url.to_string());
    }

    pub fn open_path(self: &Arc<Self>, path: &Path) {
        let resolved = resolve_local(path, &self.cwd);
        if !resolved.exists() {
            self.emit(Event::Error(format!(
                "cannot open {}: file not found",
                resolved.display()
            )));
            return;
        }
        let ext = resolved
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_ascii_lowercase());
        if let Some(ext) = ext
            && let Some(key) = self
                .config()
                .open
                .iter()
                .find(|k| k.eq_ignore_ascii_case(&ext))
                .cloned()
        {
            self.run_open_callback(key, resolved);
            return;
        }
        self.spawn_os_open(resolved.display().to_string());
    }

    fn run_open_callback(self: &Arc<Self>, ext: String, path: PathBuf) {
        let cancel = Arc::new(AtomicBool::new(false));
        {
            let mut list = self.cmd_runs.lock().unwrap();
            list.retain(|c| !c.load(Ordering::Relaxed));
            list.push(cancel.clone());
        }
        let stop = Stop::new(cancel.clone(), None);
        let weak = Arc::downgrade(self);
        let exec_id = self.next_lua_id();
        let started = Instant::now();
        self.lua_start_hook(exec_id.clone(), "open", None, Some(ext.clone()));
        let path_str = path.display().to_string();
        self.vm.post(move |lua| {
            let (s2, e2, flag, id2) = (weak.clone(), ext.clone(), cancel.clone(), exec_id.clone());
            let done = move |_: &Lua, d: Done| {
                flag.store(true, Ordering::Relaxed);
                if let Some(s) = s2.upgrade() {
                    s.lua_end_done(&id2, "open", started.elapsed(), &d);
                    if !d.ok && d.stop != Some("cancelled") {
                        s.emit(Event::Error(format!(
                            "open.{e2}: {}",
                            crate::host::error_line(&d.text)
                        )));
                    }
                }
            };
            let start = || -> mlua::Result<()> {
                let f = crate::config_lua::open_of(lua, &ext)
                    .ok_or_else(|| mlua::Error::runtime("open handler is gone"))?;
                let arg = Value::String(lua.create_string(&path_str)?);
                Job::new(
                    lua,
                    f,
                    MultiValue::from_vec(vec![arg]),
                    stop,
                    Out::Buf(String::new()),
                    done,
                )?
                .start(lua);
                Ok(())
            };
            if let Err(e) = start() {
                cancel.store(true, Ordering::Relaxed);
                let msg = tuse_lua::message(&e);
                if let Some(s) = weak.upgrade() {
                    s.lua_end_hook(
                        exec_id,
                        "open",
                        "error",
                        started.elapsed(),
                        None,
                        Some(msg.clone()),
                    );
                    s.emit(Event::Error(format!("open.{ext}: {msg}")));
                }
            }
        });
    }

    fn spawn_os_open(self: &Arc<Self>, target: String) {
        let s = self.clone();
        std::thread::spawn(move || {
            if let Err(e) = os_open(&target) {
                s.emit(Event::Error(format!("failed to open `{target}`: {e}")));
            }
        });
    }
}

fn resolve_local(path: &Path, cwd: &Path) -> PathBuf {
    let p = if path.is_absolute() {
        path.to_path_buf()
    } else {
        cwd.join(path)
    };
    p.canonicalize().unwrap_or(p)
}

fn os_open(target: &str) -> Result<(), String> {
    let mut cmd = if cfg!(target_os = "macos") {
        let mut c = Command::new("open");
        c.arg(target);
        c
    } else if cfg!(target_os = "windows") {
        let mut c = Command::new("cmd");
        c.args(["/C", "start", "", target]);
        c
    } else {
        let mut c = Command::new("xdg-open");
        c.arg(target);
        c
    };
    let out = cmd
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| e.to_string())?;
    if out.status.success() {
        return Ok(());
    }
    let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
    Err(if err.is_empty() {
        format!("exit status {}", out.status)
    } else {
        err
    })
}
