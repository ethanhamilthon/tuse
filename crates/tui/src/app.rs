use crate::chat::{Chat, Entry};
use crate::editor::Editor;
use crate::panel::Panel;
use crate::picker::Picker;
use crate::style::Palette;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::Receiver;
use tuse_core::{Event, InputId, Options, Session};

pub struct App {
    pub session: Arc<Session>,
    pub events: Receiver<Event>,
    pub cwd: PathBuf,
    pub config_arg: Option<PathBuf>,
    pub palette: Palette,
    pub chat: Chat,
    pub editor: Editor,
    pub picker: Option<Picker>,
    pub panel: Option<Panel>,
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
        let palette = Palette::new(&session.config().active_theme());
        let events = session.subscribe();
        let chat = Chat::from_records(&session.records());
        App {
            session,
            events,
            cwd,
            config_arg,
            palette,
            chat,
            editor: Editor::default(),
            picker: None,
            panel: None,
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

    pub fn handle_event(&mut self, e: Event) {
        match e {
            Event::User { id, text, images } => {
                if self.steering == Some(id) {
                    self.editor.clear();
                    self.steering = None;
                }
                self.chat.push(Entry::User { text, images });
                self.scroll = 0;
            }
            Event::TurnStart => {}
            Event::Text(t) => self.stream(t, false),
            Event::Reasoning(t) => self.stream(t, true),
            Event::StreamReset => {
                let mut drop: Vec<usize> = [self.streaming.take(), self.thinking.take()]
                    .into_iter()
                    .flatten()
                    .collect();
                drop.sort_unstable_by(|a, b| b.cmp(a));
                for i in drop {
                    self.chat.remove(i);
                }
            }
            Event::Assistant { text } => {
                if self.streaming.is_none() && !text.trim().is_empty() {
                    self.chat.push(Entry::Assistant(text));
                }
                self.streaming = None;
                self.thinking = None;
            }
            Event::ToolStart { id, command } => {
                self.chat.push(Entry::Tool {
                    id,
                    command,
                    output: None,
                    code: None,
                });
            }
            Event::ToolEnd { id, output, code } => {
                if let Some(Entry::Tool {
                    output: o, code: c, ..
                }) = self.chat.find_tool(&id)
                {
                    *o = Some(output);
                    *c = code;
                }
            }
            Event::Notice(t) => self.notice(t),
            Event::Error(t) => self.error(t),
            Event::TurnEnd {
                elapsed,
                calls,
                cancelled,
            } => {
                self.streaming = None;
                self.thinking = None;
                self.chat.push(Entry::TurnEnd {
                    elapsed,
                    calls,
                    cancelled,
                });
            }
            Event::Ask {
                id,
                question,
                options,
            } => {
                self.asks.push((id, question, options));
                if self.picker.as_ref().is_none_or(|p| p.kind.is_token()) {
                    self.next_ask();
                }
            }
            Event::AskDone { id } => {
                self.asks.retain(|a| a.0 != id);
                if matches!(&self.picker, Some(p) if p.kind == crate::picker::Kind::Ask(id.clone()))
                {
                    self.picker = None;
                    self.next_ask();
                }
            }
            Event::Tasks => {
                if matches!(&self.picker, Some(p) if p.kind == crate::picker::Kind::Task) {
                    self.open_tasks();
                }
            }
            Event::Usage => {}
            Event::Print { id, text } => {
                if self.chat.set_print(&id, text) {
                    self.scroll = 0;
                }
            }
            Event::Models => {
                if matches!(&self.picker, Some(p) if p.kind == crate::picker::Kind::Model) {
                    self.open_models();
                }
            }
        }
    }

    fn stream(&mut self, t: String, reasoning: bool) {
        let slot = if reasoning {
            self.thinking
        } else {
            self.streaming
        };
        if let Some(i) = slot
            && let Some(Entry::Assistant(s) | Entry::Reasoning(s)) = self.chat.get_mut(i)
        {
            s.push_str(&t);
            return;
        }
        let i = self.chat.len();
        self.chat.push(if reasoning {
            Entry::Reasoning(t)
        } else {
            Entry::Assistant(t)
        });
        if reasoning {
            self.thinking = Some(i);
        } else {
            self.streaming = Some(i);
        }
    }

    pub fn next_ask(&mut self) {
        let Some((id, question, options)) = self.asks.first().cloned() else {
            return;
        };
        let items = options
            .iter()
            .map(|o| crate::picker::item(o.clone(), "", o.clone()))
            .collect();
        self.picker = Some(Picker::new(crate::picker::Kind::Ask(id), question, items));
    }
}
