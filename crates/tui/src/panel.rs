use crate::app::App;
use crate::render::plain;
use crate::style::{Palette, Style};
use crate::text::{Line, wrap};
use crossterm::event::{KeyCode, KeyModifiers};

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
                    let clean = crate::render::clean(code);
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

impl App {
    pub(crate) fn panel_key(&mut self, code: KeyCode, mods: KeyModifiers) -> bool {
        if self.picker.is_some() || self.question.is_some() {
            return false;
        }
        let Some(panel) = &mut self.panel else {
            return false;
        };
        let step = self.palette.ui.scroll_step as isize;
        match code {
            KeyCode::Esc => {
                self.panel = None;
                self.clear_link_hover();
                if let Some((mut picker, search)) = self.trace_back.take() {
                    picker.follow(self.picker_rows());
                    self.picker = Some(picker);
                    self.editor = search;
                }
            }
            KeyCode::Up if self.editor.is_empty() => {
                panel.scroll_by(-step);
                self.clear_link_hover();
            }
            KeyCode::Down if self.editor.is_empty() => {
                panel.scroll_by(step);
                self.clear_link_hover();
            }
            KeyCode::Left
                if self.editor.is_empty() && mods.is_empty() && self.trace_back.is_some() =>
            {
                self.step_trace(-1);
            }
            KeyCode::Right
                if self.editor.is_empty() && mods.is_empty() && self.trace_back.is_some() =>
            {
                self.step_trace(1);
            }
            _ => return false,
        }
        true
    }

    fn step_trace(&mut self, delta: isize) {
        let rows = self.picker_rows();
        let Some((picker, _)) = &mut self.trace_back else {
            return;
        };
        let Some(next) = picker.selected.checked_add_signed(delta) else {
            return;
        };
        let Some(&item_idx) = picker.shown.get(next) else {
            return;
        };
        let Some(trace_idx) = picker
            .items
            .get(item_idx)
            .and_then(|it| it.value.parse::<usize>().ok())
        else {
            return;
        };
        let Some(it) = self.trace_items.get(trace_idx) else {
            return;
        };
        picker.selected = next;
        picker.follow(rows);
        self.panel = Some(trace_panel(it));
        self.selection = Default::default();
        self.clear_link_hover();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::App;
    use crate::picker::{Kind, Picker, item};
    use crossterm::event::KeyEvent;
    use tuse_config::{Theme, UiSettings};
    use tuse_core::TraceItem;

    fn press(app: &mut App, code: KeyCode, mods: KeyModifiers) {
        app.key(KeyEvent::new(code, mods), 10);
    }

    fn open_trace_with(app: &mut App, items: Vec<TraceItem>) {
        let rows: Vec<_> = items
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
        let lines = panel.lines(20, 8, &p);
        assert_eq!(lines.len(), 8);
        panel.scroll_by(1000);
        let last = panel.lines(20, 8, &p);
        assert_eq!(panel.scroll, 30 - 7);
        assert_eq!(last.len(), 8);
        let short = Panel::new("one").lines(20, 8, &p);
        assert_eq!(short.len(), 2);
    }

    #[test]
    fn trace_arrows_step_filtered_entries_reset_scroll_clamp_ends_and_gate_editor() {
        let mut app = crate::app::test_app("trace-arrows");
        app.paste("orig draft");
        let long = (0..25)
            .map(|i| format!("line-{i}"))
            .collect::<Vec<_>>()
            .join("\n");
        let items = vec![
            TraceItem {
                role: "system",
                summary: "sys".into(),
                text: format!("# Title\n\n{long}"),
            },
            TraceItem {
                role: "output",
                summary: "out".into(),
                text: format!("plain-out\n{long}"),
            },
            TraceItem {
                role: "lua",
                summary: "first lua".into(),
                text: format!("local a = 1\n--{long}"),
            },
            TraceItem {
                role: "user",
                summary: "hi".into(),
                text: "hello".into(),
            },
            TraceItem {
                role: "lua",
                summary: "second lua".into(),
                text: format!(
                    "local b = 2\n--b-body\n{}",
                    (0..25)
                        .map(|i| format!("--b-{i}"))
                        .collect::<Vec<_>>()
                        .join("\n")
                ),
            },
        ];
        open_trace_with(&mut app, items.clone());
        press(&mut app, KeyCode::Enter, KeyModifiers::NONE);
        app.panel.as_mut().unwrap().scroll_by(5);
        press(&mut app, KeyCode::Left, KeyModifiers::NONE);
        assert_eq!(app.panel.as_ref().unwrap().scroll, 5);
        assert!(panel_text(&mut app).contains("line-5"));

        app.hover = Some(1);
        press(&mut app, KeyCode::Right, KeyModifiers::NONE);
        assert_eq!(app.panel.as_ref().unwrap().scroll, 0);
        assert_eq!(app.hover, None);
        assert!(panel_text(&mut app).contains("plain-out"));

        press(&mut app, KeyCode::Right, KeyModifiers::NONE);
        let lua_lines = app.panel.as_mut().unwrap().lines(60, 16, &app.palette);
        assert!(
            lua_lines
                .iter()
                .flatten()
                .any(|s| s.text == "local" && s.style.fg == app.palette.err)
        );
        app.panel.as_mut().unwrap().scroll_by(4);
        press(&mut app, KeyCode::Left, KeyModifiers::NONE);
        assert_eq!(app.panel.as_ref().unwrap().scroll, 0);
        assert!(panel_text(&mut app).contains("plain-out"));

        app.paste("ab");
        press(&mut app, KeyCode::Left, KeyModifiers::NONE);
        assert_eq!(app.editor.cursor, 1);
        assert!(panel_text(&mut app).contains("plain-out"));
        press(&mut app, KeyCode::Right, KeyModifiers::SHIFT);
        assert_eq!(app.editor.selected_text().as_deref(), Some("b"));
        app.editor.clear();
        press(&mut app, KeyCode::Right, KeyModifiers::SHIFT);
        assert!(panel_text(&mut app).contains("plain-out"));

        press(&mut app, KeyCode::Esc, KeyModifiers::NONE);
        assert_eq!(app.picker.as_ref().unwrap().selected, 1);
        press(&mut app, KeyCode::Esc, KeyModifiers::NONE);
        assert_eq!(app.editor.text(), "orig draft");

        open_trace_with(&mut app, items);
        for c in "lua".chars() {
            press(&mut app, KeyCode::Char(c), KeyModifiers::NONE);
        }
        press(&mut app, KeyCode::Enter, KeyModifiers::NONE);
        assert!(panel_text(&mut app).contains("local a = 1"));
        press(&mut app, KeyCode::Right, KeyModifiers::NONE);
        assert!(panel_text(&mut app).contains("local b = 2"));
        app.panel.as_mut().unwrap().scroll_by(6);
        press(&mut app, KeyCode::Right, KeyModifiers::NONE);
        assert_eq!(app.panel.as_ref().unwrap().scroll, 6);
        assert!(panel_text(&mut app).contains("--b-4"));
        press(&mut app, KeyCode::Esc, KeyModifiers::NONE);
        assert_eq!(app.editor.text(), "lua");
        assert_eq!(app.picker.as_ref().unwrap().selected, 1);
        press(&mut app, KeyCode::Esc, KeyModifiers::NONE);
        assert_eq!(app.editor.text(), "orig draft");

        app.run_command("session");
        assert!(app.panel.is_some() && app.trace_back.is_none());
        press(&mut app, KeyCode::Right, KeyModifiers::NONE);
        assert!(panel_text(&mut app).contains("ID"));
        press(&mut app, KeyCode::Esc, KeyModifiers::NONE);
        assert!(app.panel.is_none());

        let many: Vec<_> = (0..12)
            .map(|i| TraceItem {
                role: "output",
                summary: format!("item {i}"),
                text: format!("body-{i}"),
            })
            .collect();
        app.palette.ui.list_max_height = 4;
        app.list_rows = 3;
        open_trace_with(&mut app, many);
        press(&mut app, KeyCode::Enter, KeyModifiers::NONE);
        for _ in 0..7 {
            press(&mut app, KeyCode::Right, KeyModifiers::NONE);
        }
        assert!(panel_text(&mut app).contains("body-7"));
        press(&mut app, KeyCode::Esc, KeyModifiers::NONE);
        let p = app.picker.as_ref().unwrap();
        assert_eq!(p.selected, 7);
        assert!(p.selected >= p.scroll && p.selected < p.scroll + 3);
        press(&mut app, KeyCode::Esc, KeyModifiers::NONE);

        open_trace_with(
            &mut app,
            vec![
                TraceItem {
                    role: "output",
                    summary: "first".into(),
                    text: "entry-zero".into(),
                },
                TraceItem {
                    role: "output",
                    summary: "second".into(),
                    text: "entry-one".into(),
                },
            ],
        );
        press(&mut app, KeyCode::Enter, KeyModifiers::NONE);
        assert!(panel_text(&mut app).contains("entry-zero"));
        app.open_models();
        assert!(app.picker.is_some() && app.editor.is_empty());
        press(&mut app, KeyCode::Right, KeyModifiers::NONE);
        press(&mut app, KeyCode::Left, KeyModifiers::NONE);
        press(&mut app, KeyCode::Esc, KeyModifiers::NONE);
        assert!(app.picker.is_none());
        assert!(panel_text(&mut app).contains("entry-zero"));
        assert_eq!(app.trace_back.as_ref().unwrap().0.selected, 0);
        crate::app::cleanup(app);
    }
}
