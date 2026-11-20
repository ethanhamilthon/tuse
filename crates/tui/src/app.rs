use crate::chat::{Chat, Entry};
use crate::editor::Editor;
use crate::panel::Panel;
use crate::picker::Picker;
use crate::style::Palette;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::Receiver;
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
    pub picker: Option<Picker>,
    pub panel: Option<Panel>,
    /// The `/trace` item shown in the panel; Esc goes back to the list at this item.
    pub trace_item: Option<usize>,
    /// `(role, text)` of every `/trace` item.
    pub trace: Vec<(&'static str, String)>,
    /// Rows the open list shows (set while drawing); the keys keep the selection in view.
    pub list_rows: usize,
    pub input_area: Option<InputArea>,
    /// The token whose picker was dismissed with Esc; it stays closed until the token changes.
    pub dismissed: Option<String>,
    pub asks: Vec<(String, String, Vec<String>)>,
    /// The directory the `@` picker lists, relative to `cwd`.
    pub file_dir: String,
    pub streaming: Option<usize>,
    pub thinking: Option<usize>,
    /// Lines scrolled up from the bottom of the chat.
    pub scroll: usize,
    pub tick: usize,
    /// Width of the input text area at the last draw; the arrows move by rows of this width.
    pub input_cols: usize,
    /// The queued message whose text is still shown in the input.
    pub steering: Option<InputId>,
    pub quit: bool,
}

impl App {
    pub fn new(session: Arc<Session>, cwd: PathBuf, config_arg: Option<PathBuf>) -> App {
        let palette = Palette::from_config(&session.config());
        let events = session.subscribe();
        let chat = Chat::from_records(&session.records());
        App {
            session,
            events,
            cwd,
            config_arg,
            palette,
            palette_gen: 0,
            chat,
            editor: Editor::default(),
            picker: None,
            panel: None,
            trace_item: None,
            trace: vec![],
            list_rows: 8,
            input_area: None,
            dismissed: None,
            asks: vec![],
            file_dir: String::new(),
            streaming: None,
            thinking: None,
            scroll: 0,
            tick: 0,
            input_cols: 80,
            steering: None,
            quit: false,
        }
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
        self.session.close();
        let opened = Session::open(Options {
            cwd: self.cwd.clone(),
            config: self.config_arg.clone(),
            headless: false,
            resume: resume.clone(),
        });
        let (session, err) = match opened {
            Ok(s) => (s, None),
            Err(e) => {
                let fresh = Session::open(Options {
                    cwd: self.cwd.clone(),
                    config: self.config_arg.clone(),
                    headless: false,
                    resume: None,
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

    pub fn next_ask(&mut self) {
        let Some((id, question, options)) = self.asks.first().cloned() else {
            return;
        };
        let mut items: Vec<_> = options
            .iter()
            .map(|o| crate::picker::item(o.clone(), "", o.clone()))
            .collect();
        // The last row is a reminder that the user can write their own answer; its value is empty.
        items.push(crate::picker::item(crate::style::ASK_CUSTOM_LABEL, "", ""));
        self.picker = Some(Picker::new(crate::picker::Kind::Ask(id), question, items));
    }
}
