use std::io::Read;
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Instant;

pub struct ShOut {
    pub out: String,
    pub code: Option<i32>,
    pub timed_out: bool,
}

pub struct Proc {
    child: Child,
    buf: Arc<Mutex<Vec<u8>>>,
    reader: Option<JoinHandle<()>>,
    deadline: Option<Instant>,
}

impl Proc {
    pub fn start(
        cmd: &str,
        cwd: &Path,
        env: &[(String, String)],
        deadline: Option<Instant>,
    ) -> Result<Self, String> {
        let fail = |e: std::io::Error| format!("sh: cannot start bash: {e}");
        let (mut reader, writer) = std::io::pipe().map_err(fail)?;
        let mut c = Command::new("bash");
        c.arg("-c")
            .arg(cmd)
            .current_dir(cwd)
            .stdin(Stdio::null())
            .stdout(writer.try_clone().map_err(fail)?)
            .stderr(writer);
        unsafe {
            c.pre_exec(|| {
                if libc::setsid() == -1 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
        for (k, v) in env {
            c.env(k, v);
        }
        let child = c.spawn().map_err(fail)?;
        drop(c);
        let buf = Arc::new(Mutex::new(Vec::new()));
        let into = buf.clone();
        let reader = std::thread::spawn(move || {
            let mut chunk = [0u8; 8192];
            while let Ok(n) = reader.read(&mut chunk) {
                if n == 0 {
                    break;
                }
                into.lock().unwrap().extend_from_slice(&chunk[..n]);
            }
        });
        Ok(Self {
            child,
            buf,
            reader: Some(reader),
            deadline,
        })
    }

    pub fn poll(&mut self) -> Option<ShOut> {
        let timed_out = self.deadline.is_some_and(|d| Instant::now() >= d);
        let status = match self.child.try_wait() {
            Ok(Some(s)) => Some(s),
            Ok(None) if timed_out => {
                self.kill();
                None
            }
            Ok(None) => return None,
            Err(_) => None,
        };
        Some(self.finish(status.and_then(|s| s.code()), timed_out))
    }

    pub fn wait(mut self, stop: impl Fn() -> bool) -> ShOut {
        loop {
            if stop() {
                self.kill();
                return self.finish(None, true);
            }
            if let Some(out) = self.poll() {
                return out;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }

    pub fn kill(&mut self) {
        kill_group(self.child.id());
        let _ = self.child.wait();
    }

    fn finish(&mut self, code: Option<i32>, timed_out: bool) -> ShOut {
        if let Some(r) = self.reader.take() {
            let until = Instant::now() + std::time::Duration::from_millis(200);
            while !r.is_finished() && Instant::now() < until {
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
        }
        let bytes = std::mem::take(&mut *self.buf.lock().unwrap());
        ShOut {
            out: String::from_utf8_lossy(&bytes).into_owned(),
            code,
            timed_out,
        }
    }
}

impl Drop for Proc {
    fn drop(&mut self) {
        if matches!(self.child.try_wait(), Ok(None)) {
            self.kill();
        }
    }
}

pub fn setsid() -> std::io::Result<()> {
    if unsafe { libc::setsid() } == -1 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(())
}

pub fn kill_group(pid: u32) {
    if let Ok(pid) = libc::pid_t::try_from(pid) {
        unsafe {
            libc::kill(-pid, libc::SIGKILL);
        }
    }
}
