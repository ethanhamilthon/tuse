use std::io::Read;
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
}

/// Bytes of output `sh` keeps; the rest is counted and dropped.
const CAPTURE: usize = 16 * 1024 * 1024;

/// Runs `bash -c cmd`, stdout and stderr merged. The process group is killed on
/// timeout or when `cancel` is set.
pub fn run(
    cmd: &str,
    cwd: &Path,
    env: &[(String, String)],
    timeout: Duration,
    cancel: &AtomicBool,
) -> Sh {
    let fail = |e: std::io::Error| Sh {
        out: format!("failed to start bash: {e}"),
        code: None,
        timed_out: false,
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
    let mut child = match c.spawn() {
        Ok(ch) => ch,
        Err(e) => return fail(e),
    };
    drop(c);
    let buf = Arc::new(Mutex::new((Vec::new(), 0usize)));
    let sink = buf.clone();
    let (done_tx, done_rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut chunk = [0u8; 8192];
        while let Ok(n) = reader.read(&mut chunk) {
            if n == 0 {
                break;
            }
            let mut b = sink.lock().unwrap();
            let room = CAPTURE.saturating_sub(b.0.len()).min(n);
            b.0.extend_from_slice(&chunk[..room]);
            b.1 += n - room;
        }
        let _ = done_tx.send(());
    });
    let start = Instant::now();
    let mut timed_out = false;
    let status = loop {
        if let Ok(Some(s)) = child.try_wait() {
            break Some(s);
        }
        let cancelled = cancel.load(Ordering::Relaxed);
        timed_out = !cancelled && start.elapsed() >= timeout;
        if timed_out || cancelled {
            kill_group(child.id());
            let _ = child.wait();
            break None;
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    // Background children may keep the pipe open; do not wait for them long.
    let _ = done_rx.recv_timeout(Duration::from_millis(500));
    let (bytes, dropped) = buf.lock().unwrap().clone();
    let mut out = String::from_utf8_lossy(&bytes).into_owned();
    if dropped > 0 {
        out.push_str(&format!("\n[... {dropped} more bytes dropped]"));
    }
    Sh {
        out,
        code: status.and_then(|s| s.code()),
        timed_out,
    }
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
