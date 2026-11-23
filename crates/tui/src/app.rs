use crate::chat::{Chat, Entry};
use crate::editor::Editor;
use crate::panel::Panel;
use crate::picker::Picker;
use crate::style::Palette;
use std::path::PathBuf;
use std::sync::{Arc, mpsc::Receiver};
use tuse_core::{Event, InputId, Session};

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

pub struct QuestionPrompt {
    pub id: u64,
    pub panel: Panel,
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
    pub question: Option<QuestionPrompt>,
    /// Snapshot of `/trace` items and the picker state to return to on Esc from a detail view.
    pub trace_items: Vec<tuse_core::TraceItem>,
    pub trace_back: Option<(Picker, Editor)>,
    /// Rows the open list shows (set while drawing); the keys keep the selection in view.
    pub list_rows: usize,
    pub input_area: Option<InputArea>,
    /// Screen rows occupied by the picker or panel at the last draw.
    pub overlay_rows: std::ops::Range<usize>,
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
}

impl App {
    pub fn new(session: Arc<Session>, config_arg: Option<PathBuf>) -> App {
        let cwd = session.cwd.clone();
        let palette = Palette::for_session(&session.config(), &cwd);
        let events = session.subscribe();
        let chat = Chat::from_session(&session);
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
            question: None,
            trace_items: vec![],
            trace_back: None,
            list_rows: 8,
            input_area: None,
            overlay_rows: 0..0,
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
        }
    }

    pub fn scroll_history(&mut self, delta: isize) {
        self.scroll = self.scroll.saturating_add_signed(delta);
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

    /// Switches to a new (`resume: None`) or existing session; on failure the current
    /// session stays open and the error is shown.
    pub fn switch(&mut self, resume: Option<String>) {
        match self.session.switch(resume) {
            Ok(s) => *self = App::new(s, self.config_arg.clone()),
            Err(e) => self.error(e),
        }
    }
}

/// Points `HOME` at a temporary folder once, before any test opens a session.
#[cfg(test)]
pub fn test_home() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let home = std::env::temp_dir().join(format!("tuse-home-{}", std::process::id()));
        std::fs::create_dir_all(&home).unwrap();
        // SAFETY: every test that reads the environment calls this first and waits for it.
        unsafe {
            std::env::set_var("HOME", &home);
        }
    });
}

#[cfg(test)]
pub fn test_app(name: &str) -> App {
    test_home();
    let cwd = std::env::temp_dir().join(format!("tuse-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&cwd).unwrap();
    let s = Session::open(tuse_core::Options {
        cwd: cwd.clone(),
        mode: tuse_core::Mode::Tui,
        start: Default::default(),
        config: None,
        resume: None,
    })
    .unwrap();
    App::new(s, None)
}
#[cfg(test)]
pub fn cleanup(app: App) {
    app.session.close();
    std::fs::remove_dir_all(tuse_core::tuse_home().join(&app.session.id)).ok();
    std::fs::remove_dir_all(app.cwd).ok();
}
