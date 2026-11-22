//! A small Python tokenizer for coloring code. It never fails: broken or
//! half-written code (a call is shown while it streams) is colored as far as it goes.
use crate::style::{Palette, Style};
use crate::text::{Line, span};

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Kind {
    Plain,
    Keyword,
    Str,
    Comment,
    Number,
    Function,
}

const KEYWORDS: &[&str] = &[
    "False", "None", "True", "and", "as", "assert", "async", "await", "break", "class", "continue",
    "def", "del", "elif", "else", "except", "finally", "for", "from", "global", "if", "import",
    "in", "is", "lambda", "nonlocal", "not", "or", "pass", "raise", "return", "try", "while",
    "with", "yield",
];

/// The host functions of the `python` tool.
const FUNCTIONS: &[&str] = &[
    "sh",
    "read",
    "write",
    "edit",
    "grep",
    "task_run",
    "task_add",
    "task_get",
    "task_stop",
    "task_list",
    "docs",
];

type Runs = Vec<(Kind, String)>;

/// Splits one line into runs. `open` is the quote of a triple-quoted string left open by
/// the line before (empty when none); it is updated for the next line.
pub fn tokens(line: &str, open: &mut String) -> Runs {
    let c: Vec<char> = line.chars().collect();
    let mut out: Runs = vec![];
    let mut i = 0;
    if !open.is_empty() {
        let (text, closed) = body(&c, 0, open);
        i = text.chars().count();
        out.push((Kind::Str, text));
        if !closed {
            return out;
        }
        open.clear();
    }
    while i < c.len() {
        let ch = c[i];
        let start = i;
        let kind = if ch == '#' {
            i = c.len();
            Kind::Comment
        } else if ch.is_ascii_digit()
            || (ch == '.' && c.get(i + 1).is_some_and(char::is_ascii_digit))
        {
            i = scan(&c, i, |x| x.is_ascii_alphanumeric() || x == '.' || x == '_');
            Kind::Number
        } else if ch == '"' || ch == '\'' {
            i = string(&c, i, open);
            Kind::Str
        } else if ch.is_alphabetic() || ch == '_' {
            i = scan(&c, i, |x| x.is_alphanumeric() || x == '_');
            let word: String = c[start..i].iter().collect();
            let prefix = word.len() <= 2 && word.chars().all(|x| "rbfuRBFU".contains(x));
            if prefix && matches!(c.get(i), Some('"' | '\'')) {
                i = string(&c, i, open);
                Kind::Str
            } else if KEYWORDS.contains(&word.as_str()) {
                Kind::Keyword
            } else if FUNCTIONS.contains(&word.as_str()) || c.get(i) == Some(&'(') {
                Kind::Function
            } else {
                Kind::Plain
            }
        } else {
            i += 1;
            Kind::Plain
        };
        let text: String = c[start..i].iter().collect();
        match out.last_mut() {
            Some((k, s)) if *k == kind && kind == Kind::Plain => s.push_str(&text),
            _ => out.push((kind, text)),
        }
    }
    out
}

fn scan(c: &[char], from: usize, keep: impl Fn(char) -> bool) -> usize {
    let mut j = from;
    while j < c.len() && keep(c[j]) {
        j += 1;
    }
    j
}

/// The end of a string whose opening quote is at `c[at]`; a triple quote left open is stored in `open`.
fn string(c: &[char], at: usize, open: &mut String) -> usize {
    let q = c[at];
    let triple = c.get(at + 1) == Some(&q) && c.get(at + 2) == Some(&q);
    let quote: String = std::iter::repeat_n(q, if triple { 3 } else { 1 }).collect();
    let from = at + quote.len();
    let (text, closed) = body(c, from, &quote);
    if !closed && triple {
        *open = quote;
    }
    from + text.chars().count()
}

/// The text of a string from `from` to its closing `quote` (included) or the end of the line.
fn body(c: &[char], from: usize, quote: &str) -> (String, bool) {
    let q: Vec<char> = quote.chars().collect();
    let mut i = from;
    while i < c.len() {
        if c[i] == '\\' {
            i += 2;
            continue;
        }
        if c[i..].starts_with(&q) {
            return (c[from..i + q.len()].iter().collect(), true);
        }
        i += 1;
    }
    (c[from.min(c.len())..].iter().collect(), false)
}

/// The colored lines of `code`, one per source line, on top of `base`.
pub fn highlight(code: &str, base: Style, p: &Palette) -> Vec<Line> {
    let mut open = String::new();
    code.split('\n')
        .map(|line| {
            tokens(line, &mut open)
                .into_iter()
                .map(|(k, s)| span(s, style(k, base, p)))
                .collect()
        })
        .collect()
}

pub fn style(k: Kind, base: Style, p: &Palette) -> Style {
    let color = match k {
        Kind::Plain => return base,
        Kind::Keyword => p.err,
        Kind::Str => p.ok,
        Kind::Comment => p.muted,
        Kind::Number => p.accent,
        Kind::Function => p.info,
    };
    Style {
        fg: color.or(base.fg),
        ..base
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn colored(line: &str) -> Vec<(Kind, String)> {
        tokens(line, &mut String::new())
            .into_iter()
            .filter(|(k, _)| *k != Kind::Plain)
            .collect()
    }

    #[test]
    fn colors_keywords_strings_comments_numbers_and_host_functions() {
        let got = colored("for x in read('a.txt'): n = 42  # note");
        let want = [
            (Kind::Keyword, "for"),
            (Kind::Keyword, "in"),
            (Kind::Function, "read"),
            (Kind::Str, "'a.txt'"),
            (Kind::Number, "42"),
            (Kind::Comment, "# note"),
        ];
        let want: Vec<(Kind, String)> = want.iter().map(|(k, s)| (*k, s.to_string())).collect();
        assert_eq!(got, want);
        // Unterminated and triple-quoted strings do not break the line or the next one.
        assert_eq!(colored("x = 'abc"), [(Kind::Str, "'abc".to_string())]);
        let mut open = String::new();
        tokens("s = \"\"\"first", &mut open);
        assert_eq!(open, "\"\"\"");
        let rest = tokens("second\"\"\" + y", &mut open);
        assert_eq!(rest[0], (Kind::Str, "second\"\"\"".to_string()));
        assert!(open.is_empty());
        assert_eq!(
            colored("f\"{a}\" + 1")[0],
            (Kind::Str, "f\"{a}\"".to_string())
        );
    }
}
