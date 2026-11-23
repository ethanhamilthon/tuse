use crate::app::App;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use tuse_config::ModelDef;

#[derive(Debug, Clone, PartialEq)]
pub enum Kind {
    Command,
    Prompt,
    File,
    Resume,
    Model,
    Effort(String),
    Task,
    Trace,
    LuaPicker(u64),
}

impl Kind {
    /// Token pickers filter by the token under the input cursor; the others
    /// are modal (searchable ones use the editor as their search box).
    pub fn is_token(&self) -> bool {
        matches!(self, Kind::Command | Kind::Prompt | Kind::File)
    }

    pub fn is_searchable(&self) -> bool {
        !self.is_token() && !matches!(self, Kind::LuaPicker(_))
    }
}

#[derive(Debug, Clone)]
pub struct Item {
    pub label: String,
    pub detail: String,
    pub value: String,
}

pub fn item(label: impl Into<String>, detail: impl Into<String>, value: impl Into<String>) -> Item {
    Item {
        label: label.into(),
        detail: detail.into(),
        value: value.into(),
    }
}

pub struct Picker {
    pub kind: Kind,
    pub items: Vec<Item>,
    pub shown: Vec<usize>,
    pub selected: usize,
    pub scroll: usize,
}

impl Picker {
    pub fn new(kind: Kind, items: Vec<Item>) -> Self {
        let shown = (0..items.len()).collect();
        Picker {
            kind,
            items,
            shown,
            selected: 0,
            scroll: 0,
        }
    }

    pub fn filter(&mut self, query: &str) {
        let query = query.to_lowercase();
        self.shown = self
            .items
            .iter()
            .enumerate()
            .filter(|(_, item)| query.is_empty() || item.label.to_lowercase().contains(&query))
            .map(|(i, _)| i)
            .collect();
        self.selected = 0;
        self.scroll = 0;
    }

    pub fn current(&self) -> Option<&Item> {
        self.shown.get(self.selected).map(|&i| &self.items[i])
    }

    pub fn select(&mut self, value: &str) {
        if let Some(pos) = self
            .shown
            .iter()
            .position(|&i| self.items[i].value == value)
        {
            self.selected = pos;
        }
    }

    /// Scrolls so that the selected row is among the `height` visible ones.
    pub fn follow(&mut self, height: usize) {
        if self.selected < self.scroll {
            self.scroll = self.selected;
        } else if self.selected >= self.scroll + height {
            self.scroll = self.selected + 1 - height;
        }
    }

    pub fn moved(&mut self, delta: isize, height: usize) {
        let n = self.shown.len();
        if n == 0 {
            return;
        }
        let next = self.selected as isize + delta;
        self.selected = next.rem_euclid(n as isize) as usize;
        self.follow(height);
    }
}

impl App {
    /// Returns true when the picker consumed the key. While a modal picker
    /// searches, text editing keys go on to the editor, which is its search box.
    pub(crate) fn picker_key(&mut self, k: KeyEvent, ctrl: bool, shift: bool) -> bool {
        let rows = self.picker_rows();
        let Some(p) = &mut self.picker else {
            return false;
        };
        match k.code {
            KeyCode::Up | KeyCode::Down if shift && p.kind.is_token() => return false,
            KeyCode::Up => p.moved(-1, rows),
            KeyCode::Down => p.moved(1, rows),
            KeyCode::Char('p') if ctrl => p.moved(-1, rows),
            KeyCode::Char('n') if ctrl => p.moved(1, rows),
            KeyCode::Enter if !shift && p.kind == Kind::File => self.pick_file(),
            KeyCode::Enter if !shift => self.choose(),
            KeyCode::Tab if !shift && !matches!(p.kind, Kind::LuaPicker(_)) => self.choose(),
            KeyCode::Esc => {
                let p = self.picker.take().unwrap();
                match &p.kind {
                    k if k.is_token() => self.dismissed = Some(self.editor.token().1),
                    Kind::LuaPicker(id) => {
                        let id = *id;
                        self.restore_draft();
                        let _ = self.session.ui_reply(id, None);
                    }
                    Kind::Trace => {
                        self.trace_items.clear();
                        self.trace_back = None;
                        self.restore_draft();
                    }
                    _ => self.restore_draft(),
                }
            }
            KeyCode::Char('x') if ctrl && p.kind == Kind::Task => {
                if let Some(it) = p.current() {
                    let id = it.value.clone();
                    let _ = self.session.stop_task(&id);
                    self.open_tasks();
                }
            }
            _ if p.kind.is_token() => return false,
            _ if !p.kind.is_searchable() => return true,
            _ => return !edits_text(k),
        }
        true
    }
}

/// The keys that edit a one-line search: everything else (Shift+Enter, Ctrl+C,
/// Ctrl+D...) is swallowed so it neither adds a line nor reaches the session.
fn edits_text(k: KeyEvent) -> bool {
    let primary = k
        .modifiers
        .intersects(KeyModifiers::CONTROL | KeyModifiers::SUPER);
    match k.code {
        KeyCode::Char(c) => !primary || matches!(c, 'a' | 'e' | 'u' | 'k' | 'w' | 'v' | 'x'),
        KeyCode::Backspace
        | KeyCode::Delete
        | KeyCode::Left
        | KeyCode::Right
        | KeyCode::Home
        | KeyCode::End => true,
        _ => false,
    }
}

impl App {
    /// Every available model providers reported plus configured models.
    /// The current model stays visible even when its provider has been removed.
    pub fn open_models(&mut self) {
        let config = self.session.config();
        let (current, _) = self.session.model();
        let mut names = self.session.available_models();
        if !names.contains(&current) {
            names.insert(0, current.clone());
        }
        names.sort();
        let items: Vec<Item> = names
            .iter()
            .map(|n| {
                let def = config.model_def(n);
                model_item(def, n, self.session.model_available(n))
            })
            .collect();
        let selected = self
            .picker
            .as_ref()
            .filter(|p| p.kind == Kind::Model)
            .and_then(|p| p.current())
            .map_or(current, |i| i.value.clone());
        // A refresh keeps the search typed in the editor and re-applies it.
        self.open_modal(Picker::new(Kind::Model, items))
            .select(&selected);
    }
}

fn model_item(def: Option<&ModelDef>, name: &str, available: bool) -> Item {
    let shown = tuse_config::display_name(def, name);
    let mut parts = if shown == name {
        vec![]
    } else {
        vec![name.to_string()]
    };
    if !available && !name.is_empty() {
        parts.push("unavailable".to_string());
    }
    if let Some(m) = def {
        parts.extend(details(m));
    }
    item(shown, parts.join(" · "), name)
}

fn details(m: &ModelDef) -> Vec<String> {
    let mut out = vec![];
    if m.window > 0 {
        out.push(format!("{} ctx", tuse_core::tokens(m.window)));
    }
    if let Some(p) = &m.prices {
        out.push(format!("${}/${} per 1M", p.input, p.output));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filters_by_case_insensitive_substring_in_list_order() {
        let items = ["Mars One", "Gallery", "mars Two"].map(|s| item(s, "", s));
        let mut picker = Picker::new(Kind::File, items.into());
        picker.filter("mArS");
        assert_eq!(picker.shown, [0, 2]);
        picker.filter("m One");
        assert!(picker.shown.is_empty());
    }

    #[test]
    fn model_detail_repeats_the_name_only_under_a_display_name() {
        let opus = ModelDef {
            name: "c/opus".into(),
            window: 200_000,
            ..Default::default()
        };
        let sonnet = ModelDef {
            name: "c/sonnet".into(),
            display: "Sonnet".into(),
            window: 200_000,
            ..Default::default()
        };
        let plain = model_item(Some(&opus), "c/opus", true);
        assert_eq!(
            (plain.label.as_str(), plain.detail.as_str()),
            ("c/opus", "200k ctx")
        );
        let named = model_item(Some(&sonnet), "c/sonnet", true);
        assert_eq!(named.detail, "c/sonnet · 200k ctx");
        assert_eq!(model_item(None, "c/other", true).detail, "");
        assert_eq!(
            model_item(Some(&opus), "c/opus", false).detail,
            "unavailable · 200k ctx"
        );
    }
}
