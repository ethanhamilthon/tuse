use crate::markdown::Renderer;
use crate::style::Style;
use crate::text::{span, wrap};

impl Renderer<'_> {
    /// Lines of a fenced or indented block; `python` blocks are colored like tool calls.
    pub(crate) fn code_block(&mut self, t: &str) {
        let st = Style::fg(self.p.code_block);
        let pad = " ".repeat(self.p.t.code_indent as usize);
        let python = matches!(self.lang.as_str(), "python" | "py");
        for line in t.trim_end_matches('\n').split('\n') {
            let (mut first, _) = self.prefixes();
            first.push(span(pad.clone(), st));
            let rest = first.clone();
            let content = if python {
                crate::syntax::tokens(line, &mut self.open)
                    .into_iter()
                    .map(|(k, s)| span(s, crate::syntax::style(k, st, self.p)))
                    .collect()
            } else {
                vec![span(line, st)]
            };
            self.out.extend(wrap(&content, self.width, &first, &rest));
        }
    }
}
