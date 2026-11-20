mod app;
mod chat;
mod clipboard;
mod code_block;
mod commands;
mod draw;
mod editor;
mod events;
mod expand_prompts;
mod files;
mod fold;
mod inline;
mod input_view;
mod keys;
mod layout_keys;
mod list;
mod markdown;
mod md_stream;
mod motion;
mod mouse;
mod osc;
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
    let enabled = Arc::new(std::sync::atomic::AtomicBool::new(
        session.config().settings.ui.tab_status,
    ));
    let panic_enabled = enabled.clone();
    setup()?;
    if enabled.load(std::sync::atomic::Ordering::Relaxed) {
        let mut out = stdout();
        out.write_all(osc::SAVE.as_bytes())?;
        out.flush()?;
    }
    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        restore(panic_enabled.load(std::sync::atomic::Ordering::Relaxed));
        prev(info);
    }));
    let result = main_loop(app::App::new(session, cwd, config_arg), &enabled);
    restore(enabled.load(std::sync::atomic::Ordering::Relaxed));
    result
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

fn restore(enabled: bool) {
    let mut out = stdout();
    if enabled {
        let _ = out.write_all(osc::RESTORE.as_bytes());
    }
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

fn main_loop(
    mut app: app::App,
    enabled: &std::sync::atomic::AtomicBool,
) -> std::io::Result<String> {
    let mut out = BufWriter::with_capacity(1 << 16, stdout());
    let mut osc = osc::Osc::default();
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
        enabled.store(
            app.session.config().settings.ui.tab_status,
            std::sync::atomic::Ordering::Relaxed,
        );
        if let Some(bytes) = osc.update(&app) {
            out.write_all(&bytes)?;
            out.flush()?;
        }
        if dirty && (immediate || last_draw.elapsed() >= Duration::from_millis(style::FRAME_MS)) {
            renderer.draw(&mut app, &mut out, w, h)?;
            dirty = false;
            immediate = false;
            last_draw = Instant::now();
        }
        if app.quit {
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
