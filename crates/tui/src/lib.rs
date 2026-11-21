mod ansi;
mod app;
mod ask_view;
mod chat;
mod chat_stream;
mod children;
mod clipboard;
mod code_block;
mod commands;
mod draw;
mod editor;
mod events;
mod files;
mod fold;
mod inline;
mod input_view;
mod keys;
mod layout_keys;
mod links;
mod list;
mod markdown;
mod maxsize;
mod md_stream;
mod modal;
mod motion;
mod mouse;
mod panel;
mod panel_keys;
mod picker;
mod picker_keys;
mod pickers;
mod render;
mod selection;
mod send;
mod status;
mod style;
mod syntax;
mod table;
mod text;
mod token_picker;
mod tool_view;
mod trace_filter;
mod usage;
mod yaml_syntax;

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
use tuse_core::{Event, Session};

pub use inline::{Field, Tone, ask, say};

/// Runs the full-screen UI until the user quits; returns the last session ID.
pub fn run(
    session: Arc<Session>,
    cwd: PathBuf,
    config_arg: Option<PathBuf>,
) -> std::io::Result<String> {
    let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
    for sig in [
        signal_hook::consts::SIGINT,
        signal_hook::consts::SIGTERM,
        signal_hook::consts::SIGHUP,
    ] {
        signal_hook::flag::register(sig, stop.clone())?;
    }
    // Created before `setup` so a half-done setup is undone too.
    let _guard = RestoreGuard;
    setup()?;
    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        restore();
        prev(info);
    }));
    main_loop(app::App::new(session, cwd, config_arg), &stop)
}

/// Restores the terminal on every way out of `run` (return, `?`, unwinding).
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

fn main_loop(mut app: app::App, stop: &std::sync::atomic::AtomicBool) -> std::io::Result<String> {
    let mut out = BufWriter::with_capacity(1 << 16, stdout());
    let mut renderer = draw::Renderer::default();
    let (mut w, mut h) = terminal::size()?;
    let mut dirty = true;
    let mut immediate = true;
    let mut last_tick = Instant::now();
    let mut last_draw = Instant::now();
    loop {
        while let Ok(e) = app.events.try_recv() {
            let stream = matches!(
                e,
                Event::Text(_) | Event::Reasoning(_) | Event::ToolDelta { .. }
            );
            app.handle_event(e);
            dirty = true;
            if !stream {
                immediate = true;
            }
        }
        if app
            .flash
            .as_ref()
            .is_some_and(|(_, since)| since.elapsed() >= Duration::from_secs(style::FLASH_SECS))
        {
            app.flash = None;
            dirty = true;
        }
        if app.session.busy() && last_tick.elapsed() >= Duration::from_millis(100) {
            app.tick += 1;
            last_tick = Instant::now();
            dirty = true;
            immediate = true;
        }
        if dirty && (immediate || last_draw.elapsed() >= Duration::from_millis(style::FRAME_MS)) {
            renderer.draw(&mut app, &mut out, w, h)?;
            dirty = false;
            immediate = false;
            last_draw = Instant::now();
        }
        if app.quit || stop.load(std::sync::atomic::Ordering::Relaxed) {
            break;
        }
        if !event::poll(Duration::from_millis(style::FRAME_MS))? {
            continue;
        }
        let page = h.saturating_sub(4) as usize;
        let changed = match event::read()? {
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
    let id = app.session.id.clone();
    app.session.close();
    Ok(id)
}
