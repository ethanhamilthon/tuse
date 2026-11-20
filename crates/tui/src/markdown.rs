use crate::style::{BULLET, LIST_INDENT, Palette, QUOTE_PREFIX, RULE_CHAR, Style};
use crate::text::{Line, span, wrap};
use pulldown_cmark::{CodeBlockKind, Event, Options, Parser, Tag, TagEnd};

/// Renders Markdown into wrapped lines.
pub fn render(text: &str, width: usize, p: &Palette) -> Vec<Line> {
    let mut r = Renderer {
        p,
        width,
        out: vec![],
        cur: vec![],
        styles: vec![Style::fg(p.fg)],
        lists: vec![],
        quote: 0,
        code: false,
        lang: String::new(),
        open: String::new(),
        item_first: None,
        table: None,
    };
    let opts = Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TABLES | Options::ENABLE_TASKLISTS;
    for ev in Parser::new_ext(text, opts) {
        r.event(ev);
    }
    r.flush();
    while r.out.last().is_some_and(|l| l.is_empty()) {
        r.out.pop();
    }
    r.out
}

pub(crate) struct Renderer<'a> {
    pub(crate) p: &'a Palette,
    pub(crate) width: usize,
    pub(crate) out: Vec<Line>,
    cur: Line,
    styles: Vec<Style>,
    lists: Vec<Option<u64>>,
    quote: usize,
    code: bool,
    /// The language of the open fenced block.
    pub(crate) lang: String,
    /// A triple-quoted string left open by the line before (Python blocks).
    pub(crate) open: String,
    item_first: Option<String>,
    table: Option<crate::table::Table>,
}

impl Renderer<'_> {
    fn style(&self) -> Style {
        *self.styles.last().unwrap_or(&Style::default())
    }

    fn push_style(&mut self, f: impl FnOnce(Style) -> Style) {
        let s = f(self.style());
        self.styles.push(s);
    }

    fn indent(&self) -> String {
        " ".repeat(self.lists.len().saturating_sub(1) * LIST_INDENT)
    }

    pub(crate) fn prefixes(&mut self) -> (Line, Line) {
        let q = Style::fg(self.p.dim);
        let quote = QUOTE_PREFIX.repeat(self.quote);
        let mut first = vec![span(quote.clone(), q)];
        let mut rest = first.clone();
        if !self.lists.is_empty() {
            let marker = self.item_first.take();
            let pad = self.indent() + &" ".repeat(self.marker_width());
            match marker {
                Some(m) => first.push(span(self.indent() + &m, Style::fg(self.p.accent))),
                None => first.push(span(pad.clone(), Style::default())),
            }
            rest.push(span(pad, Style::default()));
        }
        (first, rest)
    }

    fn marker_width(&self) -> usize {
        match self.lists.last() {
            Some(Some(n)) => format!("{n}. ").len(),
            _ => crate::text::width(BULLET),
        }
    }

    fn flush(&mut self) {
        if self.cur.is_empty() && self.item_first.is_none() {
            return;
        }
        let content = std::mem::take(&mut self.cur);
        let (first, rest) = self.prefixes();
        self.out.extend(wrap(&content, self.width, &first, &rest));
    }

    fn blank(&mut self) {
        self.flush();
        if self.lists.is_empty() && self.out.last().is_some_and(|l| !l.is_empty()) {
            self.out.push(vec![]);
        }
    }

    fn text(&mut self, t: &str) {
        let st = self.style();
        self.cur.push(span(t, st));
    }

    fn event(&mut self, ev: Event) {
        match ev {
            Event::Start(tag) => self.start(tag),
            Event::End(tag) => self.end(tag),
            Event::Text(t) if self.code => self.code_block(&t),
            Event::Text(t) => self.text(&t),
            Event::Code(t) => {
                let st = Style {
                    fg: self.p.ok,
                    ..self.style()
                };
                self.cur.push(span(t.to_string(), st));
            }
            Event::SoftBreak => self.text(" "),
            Event::HardBreak => self.flush(),
            Event::Rule => {
                self.flush();
                let w = self.width.min(40);
                self.out
                    .push(vec![span(RULE_CHAR.repeat(w), Style::fg(self.p.dim))]);
                self.out.push(vec![]);
            }
            Event::TaskListMarker(done) => self.text(if done { "[x] " } else { "[ ] " }),
            Event::Html(t) | Event::InlineHtml(t) => self.text(&t),
            _ => {}
        }
    }

    fn start(&mut self, tag: Tag) {
        match tag {
            Tag::Heading { .. } => {
                self.flush();
                self.push_style(|_| Style::fg(self.p.info).bold());
            }
            Tag::Emphasis => self.push_style(|s| s.italic()),
            Tag::Strong => self.push_style(|s| s.bold()),
            Tag::Strikethrough => self.push_style(|s| Style { dim: true, ..s }),
            Tag::Link { .. } => self.push_style(|s| Style {
                fg: self.p.info,
                underline: true,
                ..s
            }),
            Tag::BlockQuote(_) => {
                self.flush();
                self.quote += 1;
            }
            Tag::CodeBlock(kind) => {
                self.flush();
                self.code = true;
                self.open.clear();
                self.lang = match &kind {
                    CodeBlockKind::Fenced(l) => {
                        l.split_whitespace().next().unwrap_or("").to_lowercase()
                    }
                    CodeBlockKind::Indented => String::new(),
                };
                if let CodeBlockKind::Fenced(lang) = kind
                    && !lang.is_empty()
                {
                    let (first, _) = self.prefixes();
                    let mut l = first;
                    l.push(span(lang.to_string(), Style::fg(self.p.dim)));
                    self.out.push(l);
                }
            }
            Tag::List(start) => {
                self.flush();
                self.lists.push(start);
            }
            Tag::Item => {
                self.flush();
                let marker = match self.lists.last_mut() {
                    Some(Some(n)) => {
                        let m = format!("{n}. ");
                        *n += 1;
                        m
                    }
                    _ => BULLET.to_string(),
                };
                self.item_first = Some(marker);
            }
            Tag::Table(align) => {
                self.flush();
                self.table = Some(crate::table::Table {
                    align,
                    ..Default::default()
                });
            }
            _ => {}
        }
    }

    fn end(&mut self, tag: TagEnd) {
        match tag {
            TagEnd::Paragraph => self.blank(),
            TagEnd::Heading(_) => {
                self.styles.pop();
                self.blank();
            }
            TagEnd::Emphasis | TagEnd::Strong | TagEnd::Strikethrough | TagEnd::Link => {
                self.styles.pop();
            }
            TagEnd::BlockQuote(_) => {
                self.flush();
                self.quote = self.quote.saturating_sub(1);
                self.blank();
            }
            TagEnd::CodeBlock => {
                self.code = false;
                self.blank();
            }
            TagEnd::List(_) => {
                self.flush();
                self.lists.pop();
                self.blank();
            }
            TagEnd::Item => self.flush(),
            TagEnd::TableCell => {
                let cell = std::mem::take(&mut self.cur);
                if let Some(t) = &mut self.table {
                    t.row.push(cell);
                }
            }
            TagEnd::TableHead | TagEnd::TableRow => {
                if let Some(t) = &mut self.table {
                    let row = std::mem::take(&mut t.row);
                    t.rows.push(row);
                }
            }
            TagEnd::Table => {
                if let Some(t) = self.table.take() {
                    let (first, rest) = self.prefixes();
                    let width = self.width.saturating_sub(
                        crate::text::line_width(&first).max(crate::text::line_width(&rest)),
                    );
                    for (i, l) in t.render(width, self.p).into_iter().enumerate() {
                        let mut row = if i == 0 { first.clone() } else { rest.clone() };
                        row.extend(l);
                        self.out.push(row);
                    }
                }
                self.blank();
            }
            _ => {}
        }
    }
}
