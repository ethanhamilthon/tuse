use crate::host::Adopt;
use std::fs::File;
use std::io::{Read, Write};
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub struct Sh {
    pub out: String,
    /// `None` when the process was killed.
    pub code: Option<i32>,
    pub timed_out: bool,
    /// The task a timed out process went on as.
    pub task: Option<String>,
}

/// Bytes of output `sh` keeps; the rest is counted and dropped.
const CAPTURE: usize = 16 * 1024 * 1024;

/// Where the output goes: memory until the process is handed over, then its log.
enum Sink {
    Buf(Vec<u8>, usize),
    Log(File),
}

/// What happens to a process that is still running at the timeout.
pub type Adopter<'a> = &'a dyn Fn(Adopt) -> Result<(String, File), Adopt>;

/// Runs `bash -c cmd`, stdout and stderr merged. The process group is killed
/// when `cancel` is set, and on timeout unless `adopt` takes the process over.
pub fn run(
    cmd: &str,
    cwd: &Path,
    env: &[(String, String)],
    timeout: Duration,
    cancel: &AtomicBool,
    adopt: Option<Adopter>,
) -> Sh {
    let fail = |e: std::io::Error| Sh {
        out: format!("failed to start bash: {e}"),
        code: None,
        timed_out: false,
        task: None,
    };
    let (mut reader, writer) = match std::io::pipe() {
        Ok(p) => p,
        Err(e) => return fail(e),
    };
    let mut c = Command::new("bash");
    c.arg("-c")
        .arg(cmd)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .process_group(0);
    for (k, v) in env {
        c.env(k, v);
    }
    match writer.try_clone() {
        Ok(w) => c.stdout(w).stderr(writer),
        Err(e) => return fail(e),
    };
    let child = match c.spawn() {
        Ok(ch) => ch,
        Err(e) => return fail(e),
    };
    drop(c);
    let sink = Arc::new(Mutex::new(Sink::Buf(Vec::new(), 0)));
    let into = sink.clone();
    let (done_tx, done_rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut chunk = [0u8; 8192];
        while let Ok(n) = reader.read(&mut chunk) {
            if n == 0 {
                break;
            }
            match &mut *into.lock().unwrap() {
                Sink::Buf(bytes, dropped) => {
                    let room = CAPTURE.saturating_sub(bytes.len()).min(n);
                    bytes.extend_from_slice(&chunk[..room]);
                    *dropped += n - room;
                }
                Sink::Log(f) => {
                    let _ = f.write_all(&chunk[..n]);
                }
            }
        }
        let _ = done_tx.send(());
    });
    let start = Instant::now();
    let mut child = Some(child);
    let (mut timed_out, mut task) = (false, None);
    let status = loop {
        let ch = child.as_mut().unwrap();
        if let Ok(Some(s)) = ch.try_wait() {
            break Some(s);
        }
        let cancelled = cancel.load(Ordering::Relaxed);
        timed_out = !cancelled && start.elapsed() >= timeout;
        if timed_out && let Some(adopt) = adopt {
            let a = Adopt {
                child: child.take().unwrap(),
                cmd: cmd.to_string(),
                cwd: cwd.to_path_buf(),
            };
            match adopt(a) {
                Ok((id, mut log)) => {
                    let mut s = sink.lock().unwrap();
                    if let Sink::Buf(bytes, _) = &*s {
                        let _ = log.write_all(bytes);
                    }
                    let (bytes, dropped) = match std::mem::replace(&mut *s, Sink::Log(log)) {
                        Sink::Buf(b, d) => (b, d),
                        Sink::Log(_) => (vec![], 0),
                    };
                    task = Some(id);
                    return Sh {
                        out: with_dropped(&bytes, dropped),
                        code: None,
                        timed_out: true,
                        task,
                    };
                }
                Err(a) => child = Some(a.child),
            }
        }
        if timed_out || cancelled {
            let ch = child.as_mut().unwrap();
            kill_group(ch.id());
            let _ = ch.wait();
            break None;
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    // Background children may keep the pipe open; do not wait for them long.
    let _ = done_rx.recv_timeout(Duration::from_millis(500));
    let (bytes, dropped) = match &*sink.lock().unwrap() {
        Sink::Buf(b, d) => (b.clone(), *d),
        Sink::Log(_) => (vec![], 0),
    };
    Sh {
        out: with_dropped(&bytes, dropped),
        code: status.and_then(|s| s.code()),
        timed_out,
        task,
    }
}

fn with_dropped(bytes: &[u8], dropped: usize) -> String {
    let mut out = String::from_utf8_lossy(bytes).into_owned();
    if dropped > 0 {
        out.push_str(&format!("\n[... {dropped} more bytes dropped]"));
    }
    out
}

pub fn kill_group(pid: u32) {
    let _ = Command::new("kill")
        .args(["-KILL", "--", &format!("-{pid}")])
        .stderr(Stdio::null())
        .status();
}

/// Keeps the head and the tail when `s` is longer than `max` bytes.
pub fn truncate(s: &str, max: usize) -> String {
    if max == 0 || s.len() <= max {
        return s.to_string();
    }
    let floor = |mut i: usize| {
        while !s.is_char_boundary(i) {
            i -= 1;
        }
        i
    };
    let head = floor(max / 2);
    let tail = floor(s.len() - max / 2);
    format!(
        "{}\n[... {} bytes cut ...]\n{}",
        &s[..head],
        tail - head,
        &s[tail..]
    )
}
