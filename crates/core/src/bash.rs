use std::io::Read;
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub struct Output {
    pub text: String,
    pub code: Option<i32>,
}

/// The `command` field of the bash tool's JSON arguments.
pub fn command_of(args: &str) -> String {
    serde_json::from_str::<serde_json::Value>(args)
        .ok()
        .and_then(|v| v["command"].as_str().map(String::from))
        .unwrap_or_else(|| args.to_string())
}

/// Runs `bash -c cmd` directly, stdout and stderr merged, killed (with its
/// process group) on timeout or cancel.
pub fn run(
    cmd: &str,
    cwd: &Path,
    env: &[(String, String)],
    timeout: Duration,
    max: usize,
    cancel: &AtomicBool,
) -> Output {
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
    let buf = Arc::new(Mutex::new(Vec::new()));
    let sink = buf.clone();
    let (done_tx, done_rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut chunk = [0u8; 8192];
        while let Ok(n) = reader.read(&mut chunk) {
            if n == 0 {
                break;
            }
            sink.lock().unwrap().extend_from_slice(&chunk[..n]);
        }
        let _ = done_tx.send(());
    });
    let start = Instant::now();
    let mut note = "";
    let status = loop {
        if let Ok(Some(s)) = child.try_wait() {
            break Some(s);
        }
        if cancel.load(Ordering::Relaxed) {
            note = "\n[cancelled]";
        } else if start.elapsed() >= timeout {
            note = "\n[timed out]";
        }
        if !note.is_empty() {
            kill_group(child.id());
            let _ = child.wait();
            break None;
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    // Background children may keep the pipe open; do not wait for them long.
    let _ = done_rx.recv_timeout(Duration::from_millis(500));
    let bytes = buf.lock().unwrap().clone();
    let mut text = truncate(&String::from_utf8_lossy(&bytes), max);
    let code = status.and_then(|s| s.code());
    text.push_str(note);
    match code {
        Some(0) => {}
        Some(c) => text.push_str(&format!("\n[exit code {c}]")),
        None if note.is_empty() => text.push_str("\n[killed by signal]"),
        None => {}
    }
    if text.trim().is_empty() {
        text = "[no output]".into();
    }
    Output { text, code }
}

pub fn kill_group(pid: u32) {
    let _ = Command::new("kill")
        .args(["-KILL", "--", &format!("-{pid}")])
        .stderr(Stdio::null())
        .status();
}

fn fail(e: std::io::Error) -> Output {
    Output {
        text: format!("failed to start bash: {e}"),
        code: None,
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runs_merges_and_times_out() {
        let no = AtomicBool::new(false);
        let d = Duration::from_secs(5);
        let o = run(
            "echo out; echo err >&2; exit 3",
            Path::new("/"),
            &[],
            d,
            1000,
            &no,
        );
        assert_eq!(o.code, Some(3));
        assert!(o.text.contains("out\nerr\n") && o.text.ends_with("[exit code 3]"));
        let t = Instant::now();
        let o = run(
            "sleep 10",
            Path::new("/"),
            &[],
            Duration::from_millis(200),
            1000,
            &no,
        );
        assert!(t.elapsed() < Duration::from_secs(3) && o.text.contains("timed out"));
        assert_eq!(truncate("abcdefghij", 4), "ab\n[... 6 bytes cut ...]\nij");
    }
}
