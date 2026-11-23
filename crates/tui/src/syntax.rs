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
    "and", "break", "do", "else", "elseif", "end", "false", "for", "function", "goto", "if", "in",
    "local", "nil", "not", "or", "repeat", "return", "then", "true", "until", "while",
];

type Runs = Vec<(Kind, String)>;

pub fn tokens(line: &str, open: &mut String) -> Runs {
    let c: Vec<char> = line.chars().collect();
    let mut out: Runs = vec![];
    let mut i = 0;
    if !open.is_empty() {
        let comment = open.starts_with('-');
        let close: Vec<char> = open.trim_start_matches('-').chars().collect();
        let (end, closed) = find(&c, 0, &close);
        out.push((
            if comment { Kind::Comment } else { Kind::Str },
            c[..end].iter().collect(),
        ));
        if !closed {
            return out;
        }
        open.clear();
        i = end;
    }
    while i < c.len() {
        let ch = c[i];
        let start = i;
        let kind = if ch == '-' && c.get(i + 1) == Some(&'-') {
            match long_open(&c, i + 2) {
                Some(close) => {
                    let (end, closed) = find(&c, i + 2, &close);
                    if !closed {
                        *open = format!("-{}", close.iter().collect::<String>());
                    }
                    i = end;
                }
                None => i = c.len(),
            }
            Kind::Comment
        } else if ch == '[' && long_open(&c, i).is_some() {
            let close = long_open(&c, i).unwrap_or_default();
            let (end, closed) = find(&c, i, &close);
            if !closed {
                *open = close.iter().collect();
            }
            i = end;
            Kind::Str
        } else if ch.is_ascii_digit()
            || (ch == '.' && c.get(i + 1).is_some_and(char::is_ascii_digit))
        {
            i = scan(&c, i, |x| x.is_ascii_alphanumeric() || x == '.' || x == '_');
            Kind::Number
        } else if ch == '"' || ch == '\'' {
            i = quoted(&c, i);
            Kind::Str
        } else if ch.is_alphabetic() || ch == '_' {
            i = scan(&c, i, |x| x.is_alphanumeric() || x == '_');
            let word: String = c[start..i].iter().collect();
            let after = c[i..].iter().find(|x| !x.is_whitespace());
            if KEYWORDS.contains(&word.as_str()) {
                Kind::Keyword
            } else if (word == "print" || tuse_core::HOST_FUNCTIONS.contains(&word.as_str()))
                || matches!(after, Some('(' | '{' | '"'))
            {
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

fn long_open(c: &[char], at: usize) -> Option<Vec<char>> {
    if c.get(at) != Some(&'[') {
        return None;
    }
    let eq = c[at + 1..].iter().take_while(|x| **x == '=').count();
    (c.get(at + 1 + eq) == Some(&'[')).then(|| {
        let mut close = vec![']'];
        close.extend(std::iter::repeat_n('=', eq));
        close.push(']');
        close
    })
}

fn find(c: &[char], from: usize, close: &[char]) -> (usize, bool) {
    let mut i = from;
    while i < c.len() {
        if !close.is_empty() && c[i..].starts_with(close) && i > from {
            return (i + close.len(), true);
        }
        i += 1;
    }
    (c.len(), false)
}

fn scan(c: &[char], from: usize, keep: impl Fn(char) -> bool) -> usize {
    let mut j = from;
    while j < c.len() && keep(c[j]) {
        j += 1;
    }
    j
}

fn quoted(c: &[char], at: usize) -> usize {
    let q = c[at];
    let mut i = at + 1;
    while i < c.len() {
        if c[i] == '\\' {
            i += 2;
            continue;
        }
        if c[i] == q {
            return i + 1;
        }
        i += 1;
    }
    c.len()
}

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
        let got = colored("for x in read('a.txt') do n = 42 end -- note");
        let want = [
            (Kind::Keyword, "for"),
            (Kind::Keyword, "in"),
            (Kind::Function, "read"),
            (Kind::Str, "'a.txt'"),
            (Kind::Keyword, "do"),
            (Kind::Number, "42"),
            (Kind::Keyword, "end"),
            (Kind::Comment, "-- note"),
        ];
        let want: Vec<(Kind, String)> = want.iter().map(|(k, s)| (*k, s.to_string())).collect();
        assert_eq!(got, want);
        assert_eq!(colored("x = 'abc"), [(Kind::Str, "'abc".to_string())]);
        let mut open = String::new();
        tokens("s = [==[first", &mut open);
        assert_eq!(open, "]==]");
        let rest = tokens("second]==] .. y", &mut open);
        assert_eq!(rest[0], (Kind::Str, "second]==]".to_string()));
        assert!(open.is_empty());
        tokens("--[[ a", &mut open);
        assert_eq!(
            tokens("b ]] x", &mut open)[0],
            (Kind::Comment, "b ]]".to_string())
        );
    }
}
