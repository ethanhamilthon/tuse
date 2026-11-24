use crate::render::plain;
use crate::style::{Palette, Style};
use crate::text::{Line, wrap};

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
    body: Body,
    pub kind: PanelKind,
    pub scroll: usize,
    cache: Option<(usize, Vec<Line>)>,
}

impl Panel {
    fn with_body(body: Body) -> Self {
        Self {
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

    fn wrapped(&mut self, width: usize, p: &Palette) -> &[Line] {
        if self.cache.as_ref().is_none_or(|(w, _)| *w != width) {
            let empty = vec![];
            let lines = match &self.body {
                Body::Text(text) => plain(text, width, Style::fg(p.fg), "", ""),
                Body::Markdown(text, true) => crate::markdown::render(text, width, p),
                Body::Markdown(text, false) => {
                    crate::markdown::render_with_links(text, width, p, false)
                }
                Body::Lua(code) => {
                    let clean = crate::text::clean(code);
                    crate::syntax::highlight(&clean, Style::fg(p.fg), p)
                        .into_iter()
                        .flat_map(|r| wrap(&r, width, &empty, &empty))
                        .collect()
                }
                Body::Styled(rows) => rows
                    .iter()
                    .flat_map(|r| {
                        let r = crate::links::linkify_line(r.clone(), p);
                        wrap(&r, width, &empty, &empty)
                    })
                    .collect(),
            };
            self.cache = Some((width, lines));
        }
        &self.cache.as_ref().unwrap().1
    }

    pub fn lines(&mut self, width: usize, max_rows: usize, p: &Palette) -> Vec<Line> {
        let inner = width.max(1);
        let body = max_rows.min(p.ui.list_max_height).max(2) - 1;
        let all = self.wrapped(inner, p).to_vec();
        let body = body.min(all.len());
        self.scroll = self.scroll.min(all.len().saturating_sub(body));
        let rows = all.into_iter().skip(self.scroll).take(body).collect();
        crate::list::framed(rows, width, p)
    }

    pub fn scroll_by(&mut self, delta: isize) {
        self.scroll = self.scroll.saturating_add_signed(delta);
    }
}

pub(crate) fn trace_panel(it: &tuse_core::TraceItem) -> Panel {
    match it.role {
        "system" | "user" | "agent" => Panel::markdown(it.text.clone()),
        "lua" => Panel::lua(it.text.clone()),
        _ => Panel::new(it.text.clone()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::App;
    use crate::picker::{Kind, Picker, item};
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use tuse_config::{Theme, UiSettings};
    use tuse_core::TraceItem;

    fn press(app: &mut App, code: KeyCode, mods: KeyModifiers) {
        app.key(KeyEvent::new(code, mods), 10);
    }

    fn open_trace_with(app: &mut App, items: Vec<TraceItem>) {
        let rows = items
            .iter()
            .enumerate()
            .map(|(i, it)| {
                item(
                    format!("{i}  {}  {}", it.role, it.summary),
                    "",
                    i.to_string(),
                )
            })
            .collect();
        app.trace_items = items;
        app.trace_back = None;
        app.open_modal(Picker::new(Kind::Trace, rows));
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
        let p = Palette::new(&Theme::default(), &UiSettings::default());
        let mut panel = Panel::new(
            (0..30)
                .map(|i| i.to_string())
                .collect::<Vec<_>>()
                .join("\n"),
        );
        assert_eq!(panel.lines(20, 8, &p).len(), 8);
        panel.scroll_by(1000);
        assert_eq!(panel.lines(20, 8, &p).len(), 8);
        assert_eq!(panel.scroll, 23);
        assert_eq!(Panel::new("one").lines(20, 8, &p).len(), 2);
    }

    #[test]
    fn trace_arrows_step_filtered_entries_reset_scroll_clamp_ends_and_gate_editor() {
        let mut app = crate::app::test_app("trace-arrows");
        let k = |app: &mut App, c| press(app, c, KeyModifiers::NONE);
        let mk = |role: &'static str, summary: &str, text: String| TraceItem {
            role,
            summary: summary.into(),
            text,
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
        open_trace_with(&mut app, items.clone());
        k(&mut app, KeyCode::Enter);
        app.panel.as_mut().unwrap().scroll_by(5);
        k(&mut app, KeyCode::Left);
        assert_eq!(app.panel.as_ref().unwrap().scroll, 5);
        assert!(panel_text(&mut app).contains("line-5"));

        app.hover = Some(1);
        k(&mut app, KeyCode::Right);
        assert_eq!((app.panel.as_ref().unwrap().scroll, app.hover), (0, None));
        assert!(panel_text(&mut app).contains("plain-out"));

        k(&mut app, KeyCode::Right);
        assert!(
            app.panel
                .as_mut()
                .unwrap()
                .lines(60, 16, &app.palette)
                .iter()
                .flatten()
                .any(|s| s.text == "local" && s.style.fg == app.palette.err)
        );
        app.panel.as_mut().unwrap().scroll_by(4);
        k(&mut app, KeyCode::Left);
        assert_eq!(app.panel.as_ref().unwrap().scroll, 0);

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

        open_trace_with(&mut app, items);
        for c in "lua".chars() {
            k(&mut app, KeyCode::Char(c));
        }
        k(&mut app, KeyCode::Enter);
        assert!(panel_text(&mut app).contains("local a = 1"));
        k(&mut app, KeyCode::Right);
        assert!(panel_text(&mut app).contains("local b = 2"));
        app.panel.as_mut().unwrap().scroll_by(6);
        k(&mut app, KeyCode::Right);
        assert_eq!(app.panel.as_ref().unwrap().scroll, 6);
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

        app.palette.ui.list_max_height = 4;
        app.list_rows = 3;
        open_trace_with(
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
        crate::app::cleanup(app);
    }
}
