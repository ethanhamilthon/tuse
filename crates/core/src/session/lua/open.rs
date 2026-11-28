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
        self.run_open_callback(url.to_string(), "url");
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
        let kind = if resolved.is_dir() { "dir" } else { "file" };
        self.run_open_callback(resolved.display().to_string(), kind);
    }

    fn run_open_callback(self: &Arc<Self>, target: String, kind: &'static str) {
        let weak = Arc::downgrade(self);
        self.vm.post(move |lua| {
            let Some(s) = weak.upgrade() else { return };
            let Some(f) = crate::scripting::registry::open_of(lua) else {
                s.spawn_os_open(target);
                return;
            };
            let cancel = Arc::new(AtomicBool::new(false));
            {
                let mut runs = s.cmd_runs.lock().unwrap();
                runs.retain(|c| !c.load(Ordering::Relaxed));
                runs.push(cancel.clone());
            }
            let code = tuse_lua::source_of(&f, None);
            let done_target = target.clone();
            let started = Instant::now();
            let stop = Stop::new(cancel.clone(), None);
            let done_cancel = cancel.clone();
            let done = move |_: &Lua, d: Done| {
                done_cancel.store(true, Ordering::Relaxed);
                if let Some(s) = weak.upgrade() {
                    let outcome = if d.ok {
                        "success"
                    } else if d.stop == Some("cancelled") {
                        "cancelled"
                    } else {
                        "error"
                    };
                    let error = (!d.ok && d.stop != Some("cancelled"))
                        .then(|| crate::text::error_line(&d.text));
                    let record = crate::Record::job(
                        "open",
                        done_target.clone(),
                        code,
                        outcome.into(),
                        started.elapsed().as_millis() as u64,
                        error.clone(),
                    );
                    s.push(&mut s.state.lock().unwrap(), record);
                    if d.ok && d.text == "fallback" {
                        s.spawn_os_open(done_target);
                    } else if let Some(error) = error {
                        s.error(format!("open: {error}"));
                    }
                }
            };
            let job = (|| -> mlua::Result<_> {
                let wrapped = lua
                    .load(
                        r#"local f = ...; return function(target, kind)
                    local handled = f(target, kind)
                    if handled == true then return 'handled' end
                    if handled == false or handled == nil then return 'fallback' end
                    error('open must return true, false, or nil')
                end"#,
                    )
                    .call::<mlua::Function>(f)?;
                let args = MultiValue::from_vec(vec![
                    Value::String(lua.create_string(&target)?),
                    Value::String(lua.create_string(kind)?),
                ]);
                Ok(Job::new(lua, wrapped, args, stop, Out::Buf(String::new()), done)?.returning())
            })();
            match job {
                Ok(job) => tuse_lua::start(lua, job),
                Err(e) => {
                    cancel.store(true, Ordering::Relaxed);
                    s.error(format!("open: {}", tuse_lua::message(&e)));
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
