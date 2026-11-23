use crate::chat::{Chat, Entry};
use crate::editor::Editor;
use crate::panel::Panel;
use crate::picker::Picker;
use crate::style::Palette;
use std::path::PathBuf;
use std::sync::{Arc, mpsc::Receiver};
use tuse_core::{Event, InputId, Options, Session};

/// Where the input text is on the screen at the last draw, for mouse clicks.
#[derive(Debug, Clone, Copy)]
pub struct InputArea {
    /// Screen row of the first visible editor row, and its first text column.
    pub row: usize,
    pub col: usize,
    pub height: usize,
    /// Index of the first visible editor row.
    pub top: usize,
    pub cols: usize,
}

pub struct App {
    pub session: Arc<Session>,
    pub events: Receiver<Event>,
    pub cwd: PathBuf,
    pub config_arg: Option<PathBuf>,
    pub palette: Palette,
    pub palette_gen: u64,
    pub chat: Chat,
    pub editor: Editor,
    pub selection: crate::selection::Selection,
    /// Links on the screen at the last draw, and the one under the pointer.
    pub links: Vec<crate::links::LinkBox>,
    pub hover: Option<u32>,
    pub picker: Option<Picker>,
    pub panel: Option<Panel>,
    /// The `/trace` item shown in the panel; Esc goes back to the list at this item.
    pub trace_item: Option<usize>,
    pub children_view: Option<String>,
    /// `(role, text)` of every `/trace` item.
    pub trace: Vec<(&'static str, String)>,
    /// Rows the open list shows (set while drawing); the keys keep the selection in view.
    pub list_rows: usize,
    pub input_area: Option<InputArea>,
    /// The token whose picker was dismissed with Esc; it stays closed until the token changes.
    pub dismissed: Option<String>,
    /// The input draft set aside while a modal picker uses the editor as its search box.
    pub saved_editor: Option<Editor>,
    /// The directory the `@` picker lists, relative to `cwd`.
    pub file_dir: String,
    pub turn_error: Option<String>,
    pub flash: Option<(String, std::time::Instant)>,
    pub pending_tools: Vec<(usize, usize)>,
    pub streaming: Option<usize>,
    /// Assistant text of the current step was already shown by streaming.
    pub streamed_text: bool,
    pub thinking: Option<usize>,
    /// Lines scrolled up from the bottom of the chat.
    pub scroll: usize,
    pub tick: usize,
    pub status: crate::status::Hold,
    /// Width of the input text area at the last draw; the arrows move by rows of this width.
    pub input_cols: usize,
    /// The queued message whose text is still shown in the input.
    pub steering: Option<InputId>,
    pub quit: bool,
    /// `/maxsize` override of the model's context window.
    pub max_size: Option<u64>,
}

impl App {
    pub fn new(session: Arc<Session>, cwd: PathBuf, config_arg: Option<PathBuf>) -> App {
        let palette = Palette::from_config(&session.config());
        let events = session.subscribe();
        let config = session.config();
        let mut chat = session.with_records(|r| Chat::from_records(r, &|m| config.display_name(m)));
        for p in session.config().problems() {
            chat.push(Entry::Notice(p));
        }
        App {
            session,
            events,
            cwd,
            config_arg,
            palette,
            palette_gen: 0,
            chat,
            editor: Editor::default(),
            selection: Default::default(),
            links: vec![],
            hover: None,
            picker: None,
            panel: None,
            trace_item: None,
            children_view: None,
            trace: vec![],
            list_rows: 8,
            input_area: None,
            dismissed: None,
            saved_editor: None,
            file_dir: String::new(),
            turn_error: None,
            flash: None,
            pending_tools: vec![],
            streaming: None,
            streamed_text: false,
            thinking: None,
            scroll: 0,
            tick: 0,
            status: Default::default(),
            input_cols: 80,
            steering: None,
            quit: false,
            max_size: None,
        }
    }

    pub fn flash(&mut self, text: impl AsRef<str>) {
        self.flash = Some((
            text.as_ref().lines().collect::<Vec<_>>().join(" "),
            std::time::Instant::now(),
        ));
    }

    pub fn notice(&mut self, text: impl Into<String>) {
        self.chat.push(Entry::Notice(text.into()));
        self.scroll = 0;
    }

    pub fn error(&mut self, text: impl Into<String>) {
        self.chat.push(Entry::Error(text.into()));
        self.scroll = 0;
    }

    /// Closes the current session and opens a new (`resume: None`) or existing one.
    pub fn switch(&mut self, resume: Option<String>) {
        let start = if resume.is_some() {
            tuse_core::Start::Resume
        } else {
            tuse_core::Start::New
        };
        self.session.close_for(start.reason());
        let opened = Session::open(Options {
            cwd: self.cwd.clone(),
            config: self.config_arg.clone(),
            mode: tuse_core::Mode::Tui,
            resume: resume.clone(),
            start,
        });
        let (session, err) = match opened {
            Ok(s) => (s, None),
            Err(e) => {
                let fresh = Session::open(Options {
                    cwd: self.cwd.clone(),
                    config: self.config_arg.clone(),
                    mode: tuse_core::Mode::Tui,
                    resume: None,
                    start: tuse_core::Start::New,
                });
                match fresh {
                    Ok(s) => (s, Some(e)),
                    Err(e2) => {
                        self.error(format!("{e}; {e2}"));
                        return;
                    }
                }
            }
        };
        let cwd = self.cwd.clone();
        let arg = self.config_arg.clone();
        *self = App::new(session, cwd, arg);
        if let Some(e) = err {
            self.error(e);
        }
    }
}

/// Points `TUSE_HOME` at a temporary folder once, before any test opens a session.
#[cfg(test)]
pub fn test_home() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let home = std::env::temp_dir().join(format!("tuse-home-{}", std::process::id()));
        // SAFETY: every test that reads the environment calls this first and waits for it.
        unsafe {
            std::env::set_var("TUSE_HOME", &home);
            std::env::set_var("TUSE_GLOBAL_CONFIG", home.join("none.lua"));
            std::env::remove_var("TUSE_PARENT");
        }
    });
}

#[cfg(test)]
pub fn test_app(name: &str) -> App {
    test_home();
    let cwd = std::env::temp_dir().join(format!("tuse-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&cwd).unwrap();
    let s = Session::open(Options {
        cwd: cwd.clone(),
        mode: tuse_core::Mode::Tui,
        start: Default::default(),
        config: None,
        resume: None,
    })
    .unwrap();
    App::new(s, cwd, None)
}
#[cfg(test)]
pub fn cleanup(app: App) {
    app.session.close();
    std::fs::remove_dir_all(tuse_core::tuse_home().join(&app.session.id)).ok();
    std::fs::remove_dir_all(app.cwd).ok();
}
