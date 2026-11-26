use super::Renderer;
use crate::kit::style::Style;
use crate::kit::text::{MessageRow, span, wrap_message};
use pulldown_cmark::CodeBlockKind;

impl Renderer<'_> {
    pub(crate) fn start_code_block(&mut self, kind: CodeBlockKind) {
        self.code = true;
        self.lua = Default::default();
        self.lang = match &kind {
            CodeBlockKind::Fenced(l) => l.split_whitespace().next().unwrap_or("").to_lowercase(),
            CodeBlockKind::Indented => String::new(),
        };
        if let CodeBlockKind::Fenced(lang) = kind
            && !lang.is_empty()
        {
            let (first, _) = self.prefixes();
            let mut l = first;
            l.push(span(lang.to_string(), Style::fg(self.p.dim)));
            self.out.push(MessageRow::decoration(l));
        }
    }

    pub(crate) fn code_block(&mut self, t: &str) {
        let st = Style::fg(self.p.fg);
        let pad = " ".repeat(crate::kit::style::CODE_INDENT);
        let lua = self.lang == "lua";
        for line in t.split_terminator('\n') {
            let (mut first, _) = self.prefixes();
            first.push(span(pad.clone(), st));
            let rest = first.clone();
            let content = if lua {
                crate::kit::rich::syntax::tokens(line, &mut self.lua)
                    .into_iter()
                    .map(|(k, s)| span(s, crate::kit::rich::syntax::style(k, st, self.p)))
                    .collect()
            } else {
                vec![span(line, st)]
            };
            let content = if self.links && !lua {
                crate::kit::links::linkify_line(content, self.p)
            } else {
                content
            };
            self.out
                .extend(wrap_message(&content, self.width, &first, &rest, 0));
        }
    }
}
