mod app;
mod chat;
mod clipboard;
mod commands;
mod draw;
mod editor;
mod files;
mod inline;
mod input_view;
mod keys;
mod markdown;
mod motion;
mod panel;
mod picker;
mod pickers;
mod render;
mod status;
mod style;
mod table;
mod text;

use crossterm::event::{
    self, DisableBracketedPaste, EnableBracketedPaste, Event as TermEvent,
    KeyboardEnhancementFlags, PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
};
use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::{cursor::SetCursorStyle, execute};
use std::io::{BufWriter, Write, stdout};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tuse_core::Session;

pub use inline::{Field, Tone, ask, say};

/// Runs the full-screen UI until the user quits; returns the last session ID.
pub fn run(
    session: Arc<Session>,
    cwd: PathBuf,
    config_arg: Option<PathBuf>,
) -> std::io::Result<String> {
    setup()?;
    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        restore();
        prev(info);
    }));
    let result = main_loop(app::App::new(session, cwd, config_arg));
    restore();
    result
}

fn setup() -> std::io::Result<()> {
    terminal::enable_raw_mode()?;
    let mut out = stdout();
    execute!(
        out,
        EnterAlternateScreen,
        EnableBracketedPaste,
        SetCursorStyle::SteadyBar
    )?;
    let _ = execute!(
        out,
        PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES)
    );
    // Alternate scroll: the mouse wheel sends Up/Down in the alternate screen.
    write!(out, "\x1b[?1007h")?;
    out.flush()
}

fn restore() {
    let mut out = stdout();
    let _ = write!(out, "\x1b[?1007l");
    let _ = execute!(
        out,
        PopKeyboardEnhancementFlags,
        DisableBracketedPaste,
        SetCursorStyle::DefaultUserShape,
        LeaveAlternateScreen,
        crossterm::cursor::Show
    );
    let _ = terminal::disable_raw_mode();
}

fn main_loop(mut app: app::App) -> std::io::Result<String> {
    let mut out = BufWriter::with_capacity(1 << 16, stdout());
    let mut dirty = true;
    let mut last_tick = Instant::now();
    loop {
        while let Ok(e) = app.events.try_recv() {
            app.handle_event(e);
            dirty = true;
        }
        if app.session.busy() && last_tick.elapsed() >= Duration::from_millis(100) {
            app.tick += 1;
            last_tick = Instant::now();
            dirty = true;
        }
        if dirty {
            let (w, h) = terminal::size()?;
            draw::draw(&mut app, &mut out, w, h)?;
            dirty = false;
        }
        if app.quit {
            break;
        }
        if !event::poll(Duration::from_millis(app.palette.t.frame_ms))? {
            continue;
        }
        let page = terminal::size()?.1.saturating_sub(4) as usize;
        match event::read()? {
            TermEvent::Key(k) => app.key(k, page),
            TermEvent::Paste(s) => app.paste(&s),
            _ => {}
        }
        dirty = true;
    }
    let id = app.session.id.clone();
    app.session.close();
    Ok(id)
}
