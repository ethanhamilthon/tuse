use crate::kit::selection::{HitRow, PANEL_BASE};
use crate::kit::style::{Palette, Style};
use crate::kit::text::{Line, MessageRow, clean, span, wrap_message};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(PANEL_BASE);

enum Body {
    Text(String),
    Markdown(String, bool),
    Lua(String),
    Styled(Vec<Line>),
}

#[derive(Clone, Copy, PartialEq)]
pub enum PanelKind {
    Plain,
    Config,
    Session,
}

pub struct Panel {
    id: u64,
    body: Body,
    pub kind: PanelKind,
    scroll: usize,
    cache: Option<(usize, Vec<MessageRow>)>,
}

fn wrapped_rows(rows: impl IntoIterator<Item = Line>, width: usize) -> Vec<MessageRow> {
    let empty = vec![];
    rows.into_iter()
        .flat_map(|r| wrap_message(&r, width, &empty, &empty, 0))
        .collect()
}

impl Panel {
    fn with_body(body: Body) -> Self {
        Self {
            id: NEXT.fetch_add(1, Ordering::Relaxed),
            body,
            kind: PanelKind::Plain,
            scroll: 0,
            cache: None,
        }
    }
    pub fn with_kind(mut self, kind: PanelKind) -> Self {
        self.kind = kind;
        self
    }
    pub fn new(value: impl Into<String>) -> Self {
        Self::with_body(Body::Text(value.into()))
    }
    pub fn markdown(value: impl Into<String>) -> Self {
        Self::with_body(Body::Markdown(value.into(), true))
    }
    pub fn markdown_unlinked(value: impl Into<String>) -> Self {
        Self::with_body(Body::Markdown(value.into(), false))
    }
    pub fn lua(value: impl Into<String>) -> Self {
        Self::with_body(Body::Lua(value.into()))
    }
    pub fn styled(value: Vec<Line>) -> Self {
        Self::with_body(Body::Styled(value))
    }
    pub fn invalidate(&mut self) {
        self.cache = None;
    }

    fn wrapped(&mut self, width: usize, p: &Palette) -> &[MessageRow] {
        if self.cache.as_ref().is_none_or(|(w, _)| *w != width) {
            let rows = match &self.body {
                Body::Text(text) => {
                    let style = Style::fg(p.fg);
                    wrapped_rows(clean(text).split('\n').map(|l| vec![span(l, style)]), width)
                }
                Body::Markdown(text, links) => {
                    crate::kit::rich::markdown::rows(text, width, p, *links)
                }
                Body::Lua(code) => wrapped_rows(
                    crate::kit::rich::syntax::highlight(&clean(code), Style::fg(p.fg), p),
                    width,
                ),
                Body::Styled(rows) => wrapped_rows(
                    rows.iter()
                        .map(|r| crate::kit::links::linkify_line(r.clone(), p)),
                    width,
                ),
            };
            self.cache = Some((width, rows));
        }
        &self.cache.as_ref().unwrap().1
    }

    pub fn rows(
        &mut self,
        width: usize,
        max_rows: usize,
        p: &Palette,
    ) -> (Vec<Line>, Vec<Option<HitRow>>) {
        let inner = width.max(1);
        let len = self.wrapped(inner, p).len();
        let body = max_rows.min(p.ui.lines.list).max(1).min(len);
        self.scroll = self.scroll.min(len.saturating_sub(body));
        let (id, scroll) = (self.id, self.scroll);
        let window = &self.wrapped(inner, p)[scroll..scroll + body];
        let hits = window
            .iter()
            .enumerate()
            .map(|(i, r)| {
                r.copy.clone().map(|copy| HitRow {
                    id,
                    index: scroll + i,
                    copy,
                })
            })
            .collect();
        let lines = window.iter().map(|r| r.line.clone()).collect();
        (crate::kit::rich::render::framed(lines, width, p), hits)
    }

    #[cfg(test)]
    pub fn lines(&mut self, width: usize, max_rows: usize, p: &Palette) -> Vec<Line> {
        self.rows(width, max_rows, p).0
    }

    #[cfg(test)]
    pub fn scroll(&self) -> usize {
        self.scroll
    }

    pub fn rebuilt(mut self, old: &Panel) -> Panel {
        self.scroll = old.scroll;
        self.id = old.id;
        self
    }

    pub fn scroll_by(&mut self, delta: isize) {
        self.scroll = self.scroll.saturating_add_signed(delta);
    }
}

pub(crate) fn history_panel(it: &tuse_core::HistoryItem) -> Panel {
    match it.role {
        "system" | "user" | "agent" => Panel::markdown(it.detail.clone()),
        "lua" => Panel::lua(it.detail.clone()),
        _ => Panel::new(it.detail.clone()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::app::App;
    use crate::ui::picker::{Kind, Picker};
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use tuse_config::Ui;
    use tuse_core::HistoryItem;

    fn press(app: &mut App, code: KeyCode, mods: KeyModifiers) {
        app.key(KeyEvent::new(code, mods), 10);
    }

    fn open_history_with(app: &mut App, items: Vec<HistoryItem>) {
        let rows = items
            .iter()
            .enumerate()
            .map(|(i, it)| crate::ui::commands::history_row(i, it))
            .collect();
        app.history_items = items;
        app.history_back = None;
        app.open_modal(Picker::new(Kind::History, rows));
    }

    fn panel_text(app: &mut App) -> String {
        app.panel
            .as_mut()
            .unwrap()
            .lines(60, 16, &app.palette)
            .iter()
            .flatten()
            .map(|s| s.text.as_str())
            .collect()
    }

    #[test]
    fn panel_scrolls_within_its_text_and_fits_the_rows() {
        let p = Palette::new(&Ui::default());
        let mut panel = Panel::new(
            (0..30)
                .map(|i| i.to_string())
                .collect::<Vec<_>>()
                .join("\n"),
        );
        assert_eq!(panel.lines(20, 8, &p).len(), 8);
        panel.scroll_by(1000);
        assert_eq!(panel.lines(20, 8, &p).len(), 8);
        assert_eq!(panel.scroll(), 22);
        assert_eq!(Panel::new("one").lines(20, 8, &p).len(), 1);
    }

    #[test]
    fn history_arrows_step_filtered_entries_reset_scroll_clamp_ends_and_gate_editor() {
        let mut app = crate::ui::app::test_app("history-arrows");
        let k = |app: &mut App, c| press(app, c, KeyModifiers::NONE);
        let mk = |role: &'static str, summary: &str, text: String| HistoryItem {
            role,
            summary: summary.into(),
            fields: Default::default(),
            detail: text,
        };
        app.paste("orig draft");
        let long = (0..25)
            .map(|i| format!("line-{i}"))
            .collect::<Vec<_>>()
            .join("\n");
        let b_long = (0..25)
            .map(|i| format!("--b-{i}"))
            .collect::<Vec<_>>()
            .join("\n");
        let items = vec![
            mk("system", "sys", format!("# Title\n\n{long}")),
            mk("output", "out", format!("plain-out\n{long}")),
            mk("lua", "first lua", format!("local a = 1\n--{long}")),
            mk("user", "hi", "hello".into()),
            mk(
                "lua",
                "second lua",
                format!("local b = 2\n--b-body\n{b_long}"),
            ),
        ];
        open_history_with(&mut app, items.clone());
        k(&mut app, KeyCode::Enter);
        app.panel.as_mut().unwrap().scroll_by(5);
        k(&mut app, KeyCode::Left);
        assert_eq!(app.panel.as_ref().unwrap().scroll(), 5);
        assert!(panel_text(&mut app).contains("line-5"));

        app.hover = Some(1);
        k(&mut app, KeyCode::Right);
        assert_eq!((app.panel.as_ref().unwrap().scroll(), app.hover), (0, None));
        assert!(panel_text(&mut app).contains("plain-out"));

        k(&mut app, KeyCode::Right);
        assert!(
            app.panel
                .as_mut()
                .unwrap()
                .lines(60, 16, &app.palette)
                .iter()
                .flatten()
                .any(|s| s.text == "local" && s.style.fg == app.palette.code[1])
        );
        app.panel.as_mut().unwrap().scroll_by(4);
        k(&mut app, KeyCode::Left);
        assert_eq!(app.panel.as_ref().unwrap().scroll(), 0);

        app.paste("ab");
        k(&mut app, KeyCode::Left);
        assert_eq!(app.editor.cursor(), 1);
        press(&mut app, KeyCode::Right, KeyModifiers::SHIFT);
        assert_eq!(app.editor.selected_text().as_deref(), Some("b"));
        app.editor.clear();
        press(&mut app, KeyCode::Right, KeyModifiers::SHIFT);
        assert!(panel_text(&mut app).contains("plain-out"));
        k(&mut app, KeyCode::Esc);
        assert_eq!(app.picker.as_ref().unwrap().selected, 1);
        k(&mut app, KeyCode::Esc);
        assert_eq!(app.editor.text(), "orig draft");

        open_history_with(&mut app, items);
        for c in "lua".chars() {
            k(&mut app, KeyCode::Char(c));
        }
        k(&mut app, KeyCode::Enter);
        assert!(panel_text(&mut app).contains("local a = 1"));
        k(&mut app, KeyCode::Right);
        assert!(panel_text(&mut app).contains("local b = 2"));
        app.panel.as_mut().unwrap().scroll_by(6);
        k(&mut app, KeyCode::Right);
        assert_eq!(app.panel.as_ref().unwrap().scroll(), 6);
        assert!(panel_text(&mut app).contains("--b-4"));
        k(&mut app, KeyCode::Esc);
        assert_eq!(
            (
                app.editor.text().as_str(),
                app.picker.as_ref().unwrap().selected
            ),
            ("lua", 1)
        );
        k(&mut app, KeyCode::Esc);

        app.run_command("session");
        k(&mut app, KeyCode::Right);
        assert!(panel_text(&mut app).contains("ID"));
        k(&mut app, KeyCode::Esc);

        app.palette.ui.lines.list = 4;
        app.list_rows = 3;
        open_history_with(
            &mut app,
            (0..12)
                .map(|i| mk("output", &format!("item {i}"), format!("body-{i}")))
                .collect(),
        );
        k(&mut app, KeyCode::Enter);
        for _ in 0..7 {
            k(&mut app, KeyCode::Right);
        }
        app.open_models();
        k(&mut app, KeyCode::Right);
        k(&mut app, KeyCode::Left);
        k(&mut app, KeyCode::Esc);
        assert!(panel_text(&mut app).contains("body-7"));
        k(&mut app, KeyCode::Esc);
        let p = app.picker.as_ref().unwrap();
        assert_eq!(p.selected, 7);
        assert!(p.selected >= p.scroll && p.selected < p.scroll + 3);
        crate::ui::app::cleanup(app);
    }
}
