mod frame;
mod kit;
mod ui;

use crossterm::event::{
    self, DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture,
    Event as TermEvent, KeyboardEnhancementFlags, PopKeyboardEnhancementFlags,
    PushKeyboardEnhancementFlags,
};
use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::{cursor::SetCursorStyle, execute};
use std::io::{BufWriter, Write, stdout};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tuse_core::Session;

pub fn run(session: Arc<Session>, config_arg: Option<PathBuf>) -> std::io::Result<String> {
    let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
    for sig in [
        signal_hook::consts::SIGINT,
        signal_hook::consts::SIGTERM,
        signal_hook::consts::SIGHUP,
    ] {
        signal_hook::flag::register(sig, stop.clone())?;
    }
    let _guard = RestoreGuard;
    setup()?;
    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        restore();
        prev(info);
    }));
    let dog = frame::watchdog::Watchdog::start(session.clone(), stop.clone());
    main_loop(session, config_arg, &stop, &dog)
}

struct RestoreGuard;

impl Drop for RestoreGuard {
    fn drop(&mut self) {
        restore();
    }
}

fn setup() -> std::io::Result<()> {
    terminal::enable_raw_mode()?;
    let mut out = stdout();
    execute!(
        out,
        EnterAlternateScreen,
        EnableBracketedPaste,
        EnableMouseCapture,
        SetCursorStyle::SteadyBar
    )?;
    let _ = execute!(
        out,
        PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES)
    );
    out.flush()
}

fn restore() {
    let mut out = stdout();

    let _ = execute!(
        out,
        PopKeyboardEnhancementFlags,
        DisableMouseCapture,
        DisableBracketedPaste,
        SetCursorStyle::DefaultUserShape,
        LeaveAlternateScreen,
        crossterm::cursor::Show
    );
    let _ = terminal::disable_raw_mode();
}

struct TerminalInput {
    stop: Arc<std::sync::atomic::AtomicBool>,
    worker: Option<std::thread::JoinHandle<()>>,
    events: std::sync::mpsc::Receiver<std::io::Result<TermEvent>>,
}

impl TerminalInput {
    fn start(wake: std::sync::mpsc::SyncSender<()>) -> std::io::Result<Self> {
        use std::sync::atomic::Ordering::Relaxed;
        let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let done = stop.clone();
        let (tx, events) = std::sync::mpsc::channel();
        let worker = std::thread::Builder::new()
            .name("tuse-input".into())
            .spawn(move || {
                while !done.load(Relaxed) {
                    let next = match event::poll(Duration::from_millis(50)) {
                        Ok(false) => continue,
                        Ok(true) => event::read(),
                        Err(e) => Err(e),
                    };
                    let failed = next.is_err();
                    let sent = tx.send(next).is_ok();
                    let _ = wake.try_send(());
                    if failed || !sent {
                        break;
                    }
                }
            })?;
        Ok(Self {
            stop,
            worker: Some(worker),
            events,
        })
    }
}

impl Drop for TerminalInput {
    fn drop(&mut self) {
        self.stop.store(true, std::sync::atomic::Ordering::Relaxed);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn main_loop(
    session: Arc<Session>,
    config_arg: Option<PathBuf>,
    stop: &std::sync::atomic::AtomicBool,
    dog: &frame::watchdog::Watchdog,
) -> std::io::Result<String> {
    let (wake, wake_rx) = std::sync::mpsc::sync_channel(1);
    let input = TerminalInput::start(wake.clone())?;
    let mut app = ui::app::App::with_wake(session, config_arg, Some(wake));
    let mut out = BufWriter::with_capacity(1 << 16, stdout());
    let mut renderer = frame::draw::Renderer::default();
    let (mut w, mut h) = terminal::size()?;
    let mut dirty = true;
    let mut immediate = true;
    let mut last_tick = Instant::now();
    let mut last_draw = Instant::now();
    loop {
        dog.set_session(&app.session());
        let pumped = app.pump(&mut last_tick);
        dirty |= pumped.dirty;
        immediate |= pumped.immediate;
        if dirty
            && (immediate || last_draw.elapsed() >= Duration::from_millis(kit::style::FRAME_MS))
        {
            renderer.draw(&mut app, &mut out, w, h)?;
            dirty = false;
            immediate = false;
            last_draw = Instant::now();
        }
        if app.wants_quit() || stop.load(std::sync::atomic::Ordering::Relaxed) {
            break;
        }
        let Ok(next) = input.events.try_recv() else {
            if !pumped.more {
                let _ = wake_rx.recv_timeout(redraw_wait(dirty, last_draw, last_tick, &app));
            }
            continue;
        };
        let page = app.overlay_rows.end.saturating_sub(1).max(1);
        let changed = match next? {
            TermEvent::Key(k) => {
                app.key(k, page);
                true
            }
            TermEvent::Paste(s) => {
                app.paste(&s);
                true
            }
            TermEvent::Mouse(m) => app.mouse(m),
            TermEvent::Resize(nw, nh) => {
                w = nw;
                h = nh;
                true
            }
            _ => false,
        };
        if changed {
            dirty = true;
            immediate = true;
        }
    }
    drop(input);
    let session = app.session();
    let id = app.finish();
    if !dog.claim_close() {
        loop {
            std::thread::park();
        }
    }
    session.close();
    Ok(id)
}
fn redraw_wait(
    dirty: bool,
    last_draw: Instant,
    last_tick: Instant,
    app: &ui::app::App,
) -> Duration {
    let mut wait = Duration::from_millis(250);
    if dirty {
        wait = wait
            .min(Duration::from_millis(kit::style::FRAME_MS).saturating_sub(last_draw.elapsed()));
    }
    if app.session.busy() {
        wait = wait.min(Duration::from_millis(100).saturating_sub(last_tick.elapsed()));
    }
    if !app.retry_notice_active()
        && let Some((_, since)) = &app.flash
    {
        wait =
            wait.min(Duration::from_secs(kit::style::FLASH_SECS).saturating_sub(since.elapsed()));
    }
    wait
}
