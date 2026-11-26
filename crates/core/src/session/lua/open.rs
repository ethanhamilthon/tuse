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
            self.error(format!("cannot open URL `{url}`"));
            return;
        }
        let key = self
            .config()
            .open
            .iter()
            .find(|k| k.as_str() == tuse_config::OPEN_URL)
            .cloned();
        match key {
            Some(key) => self.run_open_callback(key, url.to_string()),
            None => self.spawn_os_open(url.to_string()),
        }
    }

    pub fn open_path(self: &Arc<Self>, path: &Path) {
        let resolved = resolve_local(path, &self.cwd);
        if !resolved.exists() {
            self.error(format!(
                "cannot open {}: file not found",
                resolved.display()
            ));
            return;
        }
        let key =
            tuse_config::pick_open_key(&self.config().open, &resolved, resolved.is_dir()).cloned();
        match key {
            Some(key) => self.run_open_callback(key, resolved.display().to_string()),
            None => self.spawn_os_open(resolved.display().to_string()),
        }
    }

    fn run_open_callback(self: &Arc<Self>, ext: String, path_str: String) {
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
        self.vm.post(move |lua| {
            if let Some(s) = weak.upgrade() {
                let code = crate::scripting::registry::open_of(lua, &ext)
                    .map(|f| tuse_lua::source_of(&f, None));
                s.lua_start_hook(exec_id.clone(), "open", code, Some(ext.clone()));
            }
            let (s2, e2, flag, id2) = (weak.clone(), ext.clone(), cancel.clone(), exec_id.clone());
            let done = move |_: &Lua, d: Done| {
                flag.store(true, Ordering::Relaxed);
                if let Some(s) = s2.upgrade() {
                    s.lua_end_done(&id2, "open", started.elapsed(), &d);
                    if !d.ok && d.stop != Some("cancelled") {
                        s.error(format!("open.{e2}: {}", crate::text::error_line(&d.text)));
                    }
                }
            };
            let start = || -> mlua::Result<()> {
                let f = crate::scripting::registry::open_of(lua, &ext)
                    .ok_or_else(|| mlua::Error::runtime("open handler is gone"))?;
                let arg = Value::String(lua.create_string(&path_str)?);
                let job = Job::new(
                    lua,
                    f,
                    MultiValue::from_vec(vec![arg]),
                    stop,
                    Out::Buf(String::new()),
                    done,
                )?;
                tuse_lua::start(lua, job);
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
                    s.error(format!("open.{ext}: {msg}"));
                }
            }
        });
    }

    fn spawn_os_open(self: &Arc<Self>, target: String) {
        let s = self.clone();
        std::thread::spawn(move || {
            if let Err(e) = os_open(&target) {
                s.error(format!("failed to open `{target}`: {e}"));
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
