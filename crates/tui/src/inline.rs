//! Inline prompts for CLI subcommands: they print into the scrollback, no alternate screen.
use crate::style::{ASK_PREFIX, ERROR_PREFIX, LABEL_SEP, MASK_CHAR, NOTICE_PREFIX, Palette};
use crossterm::event::{
    self, DisableBracketedPaste, EnableBracketedPaste, Event, KeyCode, KeyEventKind, KeyModifiers,
};
use crossterm::style::{Color, Print, ResetColor, SetForegroundColor};
use crossterm::terminal::{self, Clear, ClearType};
use crossterm::{QueueableCommand, execute};
use std::io::{Result, Stdout, Write, stdout};
use tuse_config::{Theme, UiSettings};

pub struct Field<'a> {
    pub label: &'a str,
    /// Show the answer masked (a value starting with `$` stays visible).
    pub secret: bool,
    pub check: &'a dyn Fn(&str) -> std::result::Result<(), String>,
}

#[derive(Clone, Copy)]
pub enum Tone {
    Notice,
    Error,
}

struct Raw;

impl Raw {
    fn enter() -> Result<Raw> {
        terminal::enable_raw_mode()?;
        execute!(stdout(), EnableBracketedPaste)?;
        Ok(Raw)
    }
}

impl Drop for Raw {
    fn drop(&mut self) {
        let _ = execute!(stdout(), DisableBracketedPaste);
        let _ = terminal::disable_raw_mode();
    }
}

/// Prints one styled line.
pub fn say(theme: &Theme, tone: Tone, text: &str) -> Result<()> {
    let mut out = stdout();
    line(
        &mut out,
        &Palette::new(theme, &UiSettings::default()),
        tone,
        text,
        "\n",
    )?;
    out.flush()
}

fn line(out: &mut Stdout, p: &Palette, tone: Tone, text: &str, nl: &str) -> Result<()> {
    let (prefix, color) = match tone {
        Tone::Notice => (NOTICE_PREFIX, p.dim),
        Tone::Error => (ERROR_PREFIX, p.err),
    };
    paint(out, color, &format!("{prefix}{text}"))?;
    out.queue(Print(nl))?;
    Ok(())
}

fn paint(out: &mut Stdout, color: Option<Color>, text: &str) -> Result<()> {
    if let Some(c) = color {
        out.queue(SetForegroundColor(c))?;
    }
    out.queue(Print(text))?;
    out.queue(ResetColor)?;
    Ok(())
}

fn shown(f: &Field, value: &str) -> String {
    if f.secret && !value.starts_with('$') {
        MASK_CHAR.repeat(value.chars().count())
    } else {
        value.to_string()
    }
}

fn draw(out: &mut Stdout, p: &Palette, f: &Field, value: &str, last: bool) -> Result<()> {
    out.queue(Print("\r"))?;
    out.queue(Clear(ClearType::CurrentLine))?;
    paint(out, p.accent, ASK_PREFIX)?;
    paint(out, p.dim, &format!("{}{LABEL_SEP}", f.label))?;
    paint(out, p.fg, &shown(f, value))?;
    if last {
        out.queue(Print("\r\n"))?;
    }
    out.flush()
}

/// Asks the fields one by one. Enter accepts, Esc or Ctrl+C cancels (`None`).
pub fn ask(theme: &Theme, fields: &[Field]) -> Result<Option<Vec<String>>> {
    let p = Palette::new(theme, &UiSettings::default());
    let _raw = Raw::enter()?;
    let mut out = stdout();
    let mut answers = vec![];
    for f in fields {
        let mut value = String::new();
        loop {
            draw(&mut out, &p, f, &value, false)?;
            match event::read()? {
                Event::Key(k) if k.kind != KeyEventKind::Release => {
                    let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
                    match k.code {
                        KeyCode::Esc => return cancel(&mut out),
                        KeyCode::Char('c') if ctrl => return cancel(&mut out),
                        KeyCode::Char('u') if ctrl => value.clear(),
                        KeyCode::Char(c) if !ctrl => value.push(c),
                        KeyCode::Backspace => {
                            value.pop();
                        }
                        KeyCode::Enter => match (f.check)(value.trim()) {
                            Ok(()) => break,
                            Err(e) => {
                                draw(&mut out, &p, f, &value, true)?;
                                line(&mut out, &p, Tone::Error, &e, "\r\n")?;
                            }
                        },
                        _ => {}
                    }
                }
                Event::Paste(s) => value.push_str(&s.replace(['\r', '\n'], "")),
                _ => {}
            }
        }
        draw(&mut out, &p, f, &value, true)?;
        answers.push(value.trim().to_string());
    }
    Ok(Some(answers))
}

fn cancel(out: &mut Stdout) -> Result<Option<Vec<String>>> {
    out.queue(Print("\r\n"))?;
    out.flush()?;
    Ok(None)
}
