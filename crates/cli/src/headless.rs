use std::io::Write;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tuse_core::{Event, Session, UserInput};

/// `tuse -p`: prints the session ID to stderr and the assistant's replies to
/// stdout, runs the prompt to the end and exits; background tasks are stopped.
/// Exit code 1 if an error came up.
pub fn run(session: Arc<Session>, prompt: Option<String>) -> Result<(), String> {
    let stop = Arc::new(AtomicBool::new(false));
    for sig in [
        signal_hook::consts::SIGINT,
        signal_hook::consts::SIGTERM,
        signal_hook::consts::SIGHUP,
    ] {
        signal_hook::flag::register(sig, stop.clone()).map_err(|e| e.to_string())?;
    }
    eprintln!("tuse: session {}", session.id);
    for p in session.config().problems() {
        eprintln!("tuse: {p}");
    }
    let events = session.subscribe();
    let prompt = prompt.filter(|p| !p.trim().is_empty());
    if prompt.is_none() {
        session.close();
        return Err("no prompt".into());
    }
    if let Some(p) = prompt {
        session.submit(UserInput::text(p));
    }
    let mut out = std::io::stdout();
    let mut failed = false;
    while !stop.load(Ordering::Relaxed) {
        match events.recv_timeout(Duration::from_millis(100)) {
            Ok(Event::Assistant { text }) if !text.trim().is_empty() => {
                let _ = writeln!(out, "{}", text.trim_end());
                let _ = out.flush();
            }
            Ok(Event::Error(e)) => {
                eprintln!("tuse: error: {e}");
                failed = true;
            }
            Ok(Event::Notice(n) | Event::Flash(n)) => eprintln!("tuse: {n}"),
            Ok(_) => {}
            Err(_) if !session.busy() => break,
            Err(_) => {}
        }
    }
    session.close();
    if failed {
        Err("the session ended with an error".into())
    } else {
        Ok(())
    }
}
