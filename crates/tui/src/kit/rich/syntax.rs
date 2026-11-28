use crate::kit::style::{Palette, Style};
use crate::kit::text::{Line, span};

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Kind {
    Plain,
    Keyword,
    Storage,
    Constant,
    Operator,
    Punctuation,
    Number,
    Str,
    Escape,
    Comment,
    Function,
    Builtin,
    Method,
    Field,
    Parameter,
    Variable,
}

impl Kind {
    fn slot(self) -> Option<usize> {
        (self != Kind::Plain).then(|| self as usize - 1)
    }
}

const KEYWORDS: &[&str] = &[
    "break", "do", "else", "elseif", "end", "for", "goto", "if", "in", "repeat", "return", "then",
    "until", "while",
];

const STD_TABLES: &[&str] = &[
    "coroutine",
    "debug",
    "io",
    "math",
    "os",
    "string",
    "table",
    "utf8",
    "fs",
    "tasks",
    "session",
    "ui",
    "plugins",
    "providers",
    "docs",
    "json",
];

const GLOBALS: &[&str] = &[
    "_G",
    "_VERSION",
    "assert",
    "collectgarbage",
    "dofile",
    "error",
    "getmetatable",
    "ipairs",
    "load",
    "loadfile",
    "next",
    "pairs",
    "pcall",
    "print",
    "rawequal",
    "rawget",
    "rawlen",
    "rawset",
    "require",
    "select",
    "setmetatable",
    "tonumber",
    "tostring",
    "type",
    "xpcall",
];

type Runs = Vec<(Kind, String)>;

#[derive(Default, Clone, Copy, PartialEq)]
enum Def {
    #[default]
    None,
    Name,
    Params,
}

#[derive(Default, Clone, Copy, PartialEq)]
enum Access {
    #[default]
    None,
    Dot {
        std: bool,
    },
    Colon,
}

#[derive(Default)]
pub struct State {
    open: String,
    scopes: Vec<Vec<String>>,
    brackets: Vec<(char, usize)>,
    def: Def,
    access: Access,
    std: bool,
}

fn word_kind(word: &str) -> Option<Kind> {
    Some(match word {
        "local" | "function" => Kind::Storage,
        "nil" | "true" | "false" => Kind::Constant,
        "and" | "or" | "not" => Kind::Operator,
        _ if KEYWORDS.contains(&word) => Kind::Keyword,
        _ => return None,
    })
}

fn is_builtin(word: &str) -> bool {
    GLOBALS.contains(&word)
        || STD_TABLES.contains(&word)
        || tuse_core::HOST_FUNCTIONS.contains(&word)
}

fn push(out: &mut Runs, kind: Kind, text: String) {
    if text.is_empty() {
        return;
    }
    match out.last_mut() {
        Some((k, s)) if *k == kind && kind == Kind::Plain => s.push_str(&text),
        _ => out.push((kind, text)),
    }
}

fn text(c: &[char], from: usize, to: usize) -> String {
    c[from..to].iter().collect()
}

pub fn tokens(line: &str, st: &mut State) -> Runs {
    let c: Vec<char> = line.chars().collect();
    let mut out: Runs = vec![];
    let mut i = 0;
    if !st.open.is_empty() {
        let comment = st.open.starts_with('-');
        let close: Vec<char> = st.open.trim_start_matches('-').chars().collect();
        let (end, closed) = find(&c, 0, &close);
        push(
            &mut out,
            if comment { Kind::Comment } else { Kind::Str },
            text(&c, 0, end),
        );
        if !closed {
            return out;
        }
        st.open.clear();
        i = end;
    }
    while i < c.len() {
        let (ch, start) = (c[i], i);
        if ch.is_whitespace() {
            i = scan(&c, i, char::is_whitespace);
            push(&mut out, Kind::Plain, text(&c, start, i));
            continue;
        }
        if ch == '-' && c.get(i + 1) == Some(&'-') {
            match long_open(&c, i + 2) {
                Some(close) => {
                    let (end, closed) = find(&c, i + 2, &close);
                    if !closed {
                        st.open = format!("-{}", close.iter().collect::<String>());
                    }
                    i = end;
                }
                None => i = c.len(),
            }
            push(&mut out, Kind::Comment, text(&c, start, i));
            continue;
        }
        let access = std::mem::take(&mut st.access);
        let after_std = std::mem::take(&mut st.std);
        if let Some(close) = long_open(&c, i) {
            let (end, closed) = find(&c, i, &close);
            if !closed {
                st.open = close.iter().collect();
            }
            i = end;
            push(&mut out, Kind::Str, text(&c, start, i));
        } else if ch.is_ascii_digit()
            || (ch == '.' && c.get(i + 1).is_some_and(char::is_ascii_digit))
        {
            i = number_end(&c, i);
            push(&mut out, Kind::Number, text(&c, start, i));
        } else if ch == '"' || ch == '\'' {
            i = string_runs(&c, i, &mut out);
        } else if ch.is_alphabetic() || ch == '_' {
            i = scan(&c, i, |x| x.is_alphanumeric() || x == '_');
            let word = text(&c, start, i);
            let kind = classify(st, &word, access, &c, i);
            st.std = access == Access::None && STD_TABLES.contains(&word.as_str());
            push(&mut out, kind, word);
        } else {
            let (len, kind) = symbol(&c, i);
            i += len;
            let sym = text(&c, start, i);
            bracket(st, &sym, after_std);
            push(&mut out, kind, sym);
        }
    }
    out
}

fn classify(st: &mut State, word: &str, access: Access, c: &[char], at: usize) -> Kind {
    if let Some(kind) = word_kind(word) {
        match word {
            "function" => {
                st.scopes.push(vec![]);
                st.def = Def::Name;
            }
            "if" | "do" | "repeat" => st.scopes.push(vec![]),
            "end" | "until" => {
                st.scopes.pop();
            }
            _ => {}
        }
        return kind;
    }
    let next_at = c[at..]
        .iter()
        .position(|x| !x.is_whitespace())
        .map(|p| at + p);
    let next = next_at.map(|p| c[p]);
    match st.def {
        Def::Params => {
            if let Some(scope) = st.scopes.last_mut() {
                scope.push(word.to_string());
            }
            return Kind::Parameter;
        }
        Def::Name => {
            return match (next, access) {
                (Some('.' | ':'), Access::None) => Kind::Variable,
                (Some('.' | ':'), _) => Kind::Field,
                (_, Access::Colon) => Kind::Method,
                _ => Kind::Function,
            };
        }
        Def::None => {}
    }
    let call = next_at.is_some_and(|p| {
        matches!(c[p], '(' | '{' | '"' | '\'') || (c[p] == '[' && long_open(c, p).is_some())
    });
    match access {
        Access::Colon => Kind::Method,
        Access::Dot { std: true } => Kind::Builtin,
        Access::Dot { std: false } if call => Kind::Function,
        Access::Dot { std: false } => Kind::Field,
        Access::None => {
            let key = st.brackets.last() == Some(&('{', st.scopes.len()))
                && next == Some('=')
                && next_at.and_then(|p| c.get(p + 1)) != Some(&'=');
            if key {
                Kind::Field
            } else if call && is_builtin(word) {
                Kind::Builtin
            } else if call {
                Kind::Function
            } else if st.scopes.iter().any(|s| s.iter().any(|p| p == word)) {
                Kind::Parameter
            } else if is_builtin(word) {
                Kind::Builtin
            } else {
                Kind::Variable
            }
        }
    }
}

fn symbol(c: &[char], at: usize) -> (usize, Kind) {
    let rest: String = c[at..].iter().take(3).collect();
    if rest == "..." {
        return (3, Kind::Punctuation);
    }
    let two = &rest[..rest.char_indices().nth(2).map_or(rest.len(), |(i, _)| i)];
    match two {
        "==" | "~=" | "<=" | ">=" | "//" | "<<" | ">>" | ".." => return (2, Kind::Operator),
        "::" => return (2, Kind::Punctuation),
        _ => {}
    }
    match c[at] {
        '+' | '-' | '*' | '/' | '%' | '^' | '#' | '&' | '~' | '|' | '<' | '>' | '=' => {
            (1, Kind::Operator)
        }
        '(' | ')' | '{' | '}' | '[' | ']' | ';' | ',' | '.' | ':' => (1, Kind::Punctuation),
        _ => (1, Kind::Plain),
    }
}

fn bracket(st: &mut State, sym: &str, after_std: bool) {
    let depth = st.scopes.len();
    match sym {
        "(" => {
            if st.def == Def::Name {
                st.def = Def::Params;
            }
            st.brackets.push(('(', depth));
        }
        "{" | "[" => st.brackets.push((sym.chars().next().unwrap_or('{'), depth)),
        ")" => {
            st.brackets.pop();
            if st.def == Def::Params {
                st.def = Def::None;
            }
        }
        "}" | "]" => {
            st.brackets.pop();
        }
        "." => st.access = Access::Dot { std: after_std },
        ":" => st.access = Access::Colon,
        _ => {}
    }
}

fn number_end(c: &[char], from: usize) -> usize {
    let hex = c[from] == '0' && matches!(c.get(from + 1), Some('x' | 'X'));
    let mut j = from + if hex { 2 } else { 0 };
    while j < c.len() {
        let x = c[j];
        let exponent = if hex {
            matches!(x, 'p' | 'P')
        } else {
            matches!(x, 'e' | 'E')
        };
        if exponent && matches!(c.get(j + 1), Some('+' | '-')) {
            j += 2;
        } else if x.is_ascii_alphanumeric() || x == '.' || x == '_' {
            j += 1;
        } else {
            break;
        }
    }
    j
}

fn string_runs(c: &[char], at: usize, out: &mut Runs) -> usize {
    let quote = c[at];
    let (mut i, mut seg) = (at + 1, at);
    while i < c.len() {
        if c[i] == '\\' {
            push(out, Kind::Str, text(c, seg, i));
            let end = escape_end(c, i);
            push(out, Kind::Escape, text(c, i, end));
            (i, seg) = (end, end);
        } else {
            i += 1;
            if c[i - 1] == quote {
                break;
            }
        }
    }
    push(out, Kind::Str, text(c, seg, i));
    i
}

fn escape_end(c: &[char], at: usize) -> usize {
    let mut j = at + 1;
    match c.get(j) {
        None => {}
        Some('x') => {
            j += 1;
            j += c[j..]
                .iter()
                .take(2)
                .take_while(|x| x.is_ascii_hexdigit())
                .count();
        }
        Some('u') if c.get(j + 1) == Some(&'{') => {
            j = scan(c, j + 2, |x| x != '}');
            j = (j + 1).min(c.len());
        }
        Some(d) if d.is_ascii_digit() => {
            j += c[j..]
                .iter()
                .take(3)
                .take_while(|x| x.is_ascii_digit())
                .count();
        }
        Some(_) => j += 1,
    }
    j
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

pub fn highlight(code: &str, base: Style, p: &Palette) -> Vec<Line> {
    let mut state = State::default();
    code.split('\n')
        .map(|line| {
            tokens(line, &mut state)
                .into_iter()
                .map(|(k, s)| span(s, style(k, base, p)))
                .collect()
        })
        .collect()
}

pub fn style(k: Kind, base: Style, p: &Palette) -> Style {
    let Some(slot) = k.slot() else { return base };
    let color = p.code.get(slot).copied().flatten();
    Style {
        fg: color.or(base.fg),
        ..base
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use Kind::*;

    fn run(code: &str) -> Vec<(Kind, String)> {
        let mut st = State::default();
        code.split('\n')
            .flat_map(|l| tokens(l, &mut st))
            .filter(|(k, _)| *k != Plain)
            .collect()
    }

    fn pick(code: &str, kinds: &[Kind]) -> Vec<(Kind, String)> {
        run(code)
            .into_iter()
            .filter(|(k, _)| kinds.contains(k))
            .collect()
    }

    fn of(items: &[(Kind, &str)]) -> Vec<(Kind, String)> {
        items.iter().map(|(k, s)| (*k, s.to_string())).collect()
    }

    #[test]
    fn kinds_line_up_with_the_theme_tokens() {
        let names = [
            (Keyword, "keyword"),
            (Storage, "storage"),
            (Constant, "constant"),
            (Operator, "operator"),
            (Punctuation, "punctuation"),
            (Number, "number"),
            (Str, "string"),
            (Escape, "escape"),
            (Comment, "comment"),
            (Function, "function"),
            (Builtin, "builtin"),
            (Method, "method"),
            (Field, "field"),
            (Parameter, "parameter"),
            (Variable, "variable"),
        ];
        assert_eq!(names.len() + 2, tuse_config::CodeTheme::TOKENS.len());
        for (kind, name) in names {
            assert_eq!(tuse_config::CodeTheme::TOKENS[kind.slot().unwrap()], name);
        }
    }

    #[test]
    fn separates_definitions_parameters_operators_and_calls() {
        assert_eq!(
            run("local function add(a, b) return a + b end"),
            of(&[
                (Storage, "local"),
                (Storage, "function"),
                (Function, "add"),
                (Punctuation, "("),
                (Parameter, "a"),
                (Punctuation, ","),
                (Parameter, "b"),
                (Punctuation, ")"),
                (Keyword, "return"),
                (Parameter, "a"),
                (Operator, "+"),
                (Parameter, "b"),
                (Keyword, "end"),
            ])
        );
        assert_eq!(
            pick(
                "x = fs.read('a.txt'):upper() .. y",
                &[Variable, Operator, Builtin, Method]
            ),
            of(&[
                (Variable, "x"),
                (Operator, "="),
                (Builtin, "fs"),
                (Builtin, "read"),
                (Method, "upper"),
                (Operator, ".."),
                (Variable, "y"),
            ])
        );
    }

    #[test]
    fn tracks_scopes_across_lines() {
        let got = pick(
            "function M.go(self, n)\n  return n * 2\nend\nlocal v = n",
            &[Variable, Field, Function, Parameter],
        );
        assert_eq!(
            got,
            of(&[
                (Variable, "M"),
                (Function, "go"),
                (Parameter, "self"),
                (Parameter, "n"),
                (Parameter, "n"),
                (Variable, "v"),
                (Variable, "n"),
            ])
        );
        assert_eq!(
            pick("function a.b:c() end", &[Variable, Field, Method, Function]),
            of(&[(Variable, "a"), (Field, "b"), (Method, "c")])
        );
    }

    #[test]
    fn tells_table_keys_fields_and_std_members_apart() {
        assert_eq!(
            pick(
                "t = { name = 1, run = function() local y = 2 end }; t.size = string.len(t.name)",
                &[Field, Builtin, Variable]
            ),
            of(&[
                (Variable, "t"),
                (Field, "name"),
                (Field, "run"),
                (Variable, "y"),
                (Variable, "t"),
                (Field, "size"),
                (Builtin, "string"),
                (Builtin, "len"),
                (Variable, "t"),
                (Field, "name"),
            ])
        );
        assert_eq!(
            pick(
                "if a == 1 then print(a) end",
                &[Operator, Builtin, Constant]
            ),
            of(&[(Operator, "=="), (Builtin, "print")])
        );
    }

    #[test]
    fn splits_escapes_numbers_and_comments() {
        assert_eq!(
            pick("s = \"a\\n\\x41\\u{20}b\" -- note", &[Str, Escape, Comment]),
            of(&[
                (Str, "\"a"),
                (Escape, "\\n"),
                (Escape, "\\x41"),
                (Escape, "\\u{20}"),
                (Str, "b\""),
                (Comment, "-- note"),
            ])
        );
        assert_eq!(
            pick("n = 0xFF + 1e-5 + 3.14", &[Number]),
            of(&[(Number, "0xFF"), (Number, "1e-5"), (Number, "3.14")])
        );
        assert_eq!(
            pick("x = nil and true or not false", &[Constant, Operator]),
            of(&[
                (Operator, "="),
                (Constant, "nil"),
                (Operator, "and"),
                (Constant, "true"),
                (Operator, "or"),
                (Operator, "not"),
                (Constant, "false"),
            ])
        );
        assert_eq!(pick("x = 'abc", &[Str]), of(&[(Str, "'abc")]));
    }

    #[test]
    fn long_strings_and_comments_continue_across_lines() {
        let mut st = State::default();
        tokens("s = [==[first", &mut st);
        assert_eq!(st.open, "]==]");
        let rest = tokens("second]==] .. y", &mut st);
        assert_eq!(rest[0], (Str, "second]==]".to_string()));
        assert!(st.open.is_empty());
        tokens("--[[ a", &mut st);
        assert_eq!(tokens("b ]] x", &mut st)[0], (Comment, "b ]]".to_string()));
    }
}
