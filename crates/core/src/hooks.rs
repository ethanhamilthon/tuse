use std::process::{Command, Stdio};
use tuse_config::Config;

pub const EVENTS: [&str; 7] = [
    "session_start",
    "session_end",
    "turn_start",
    "turn_end",
    "tool_start",
    "tool_end",
    "error",
];

/// Starts every hook subscribed to `event` and does not wait for it.
/// Data goes through env: `TUSE_EVENT`, `TUSE_HOOK` plus `env`.
pub fn fire(config: &Config, event: &str, env: &[(String, String)]) {
    for (name, hook) in &config.hooks {
        if hook.run.is_empty() || !hook.events.iter().any(|e| e == event) {
            continue;
        }
        let mut c = Command::new("bash");
        c.arg("-c")
            .arg(&hook.run)
            .env("TUSE_EVENT", event)
            .env("TUSE_HOOK", name)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        for (k, v) in env {
            c.env(k, v);
        }
        if let Ok(mut child) = c.spawn() {
            std::thread::spawn(move || child.wait());
        }
    }
}
