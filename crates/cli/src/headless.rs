use std::io::Write;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tuse_core::{Event, Session, UserInput};

/// `tuse -p`: prints the session ID to stderr and assistant replies to stdout,
/// and keeps serving `tuse steer` until SIGINT or SIGTERM.
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
    let events = session.subscribe();
    if let Some(p) = prompt.filter(|p| !p.trim().is_empty()) {
        session.submit(UserInput::text(p));
    }
    let mut out = std::io::stdout();
    while !stop.load(Ordering::Relaxed) {
        match events.recv_timeout(Duration::from_millis(200)) {
            Ok(Event::Assistant { text }) if !text.trim().is_empty() => {
                let _ = writeln!(out, "{}", text.trim_end());
                let _ = out.flush();
            }
            Ok(Event::Error(e)) => eprintln!("tuse: error: {e}"),
            Ok(Event::Notice(n)) => eprintln!("tuse: {n}"),
            _ => {}
        }
    }
    session.close();
    Ok(())
}
