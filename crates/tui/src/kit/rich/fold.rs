use crate::kit::style::MORE;

pub fn fold_multiline_strings(code: &str, keep: usize) -> String {
    let c: Vec<char> = code.chars().collect();
    let mut out = String::with_capacity(code.len());
    let mut i = 0;
    while i < c.len() {
        let ch = c[i];
        if ch == '-' && c.get(i + 1) == Some(&'-') && long(&c, i + 2).is_none() {
            while i < c.len() && c[i] != '\n' {
                out.push(c[i]);
                i += 1;
            }
            continue;
        }
        if ch == '"' || ch == '\'' {
            out.push(ch);
            i += 1;
            while i < c.len() {
                let cur = c[i];
                out.push(cur);
                i += 1;
                if cur == '\\' && i < c.len() {
                    out.push(c[i]);
                    i += 1;
                } else if cur == ch || cur == '\n' {
                    break;
                }
            }
            continue;
        }
        let Some(eq) = long(&c, i) else {
            out.push(ch);
            i += 1;
            continue;
        };
        let open: String = c[i..i + eq + 2].iter().collect();
        let close: Vec<char> = open.replace('[', "]").chars().collect();
        let start = i + open.chars().count();
        let mut j = start;
        while j < c.len() && !c[j..].starts_with(&close) {
            j += 1;
        }
        let closed = j < c.len();
        let body: String = c[start..j].iter().collect();
        out.push_str(&open);
        if body.contains('\n') {
            out.push_str(&fold_body(&body, keep));
        } else {
            out.push_str(&body);
        }
        if closed {
            out.extend(close.iter());
            i = j + close.len();
        } else {
            i = j;
        }
    }
    out
}

fn fold_body(body: &str, keep: usize) -> String {
    let lines: Vec<&str> = body.split('\n').collect();
    let filled = |l: &&str| !l.trim().is_empty();
    let (Some(first), Some(last)) = (
        lines.iter().position(filled),
        lines.iter().rposition(filled),
    ) else {
        return MORE.to_string();
    };
    let content = &lines[first..=last];
    let shown = keep.min(content.len());
    let mut text = content[..shown]
        .iter()
        .enumerate()
        .map(|(i, l)| {
            if i == 0 && first > 0 {
                l.trim()
            } else {
                l.trim_end()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    if shown < content.len() {
        text.push_str(MORE);
    }
    text
}

fn long(c: &[char], at: usize) -> Option<usize> {
    if c.get(at) != Some(&'[') {
        return None;
    }
    let eq = c[at + 1..].iter().take_while(|x| **x == '=').count();
    (c.get(at + 1 + eq) == Some(&'[')).then_some(eq)
}

#[cfg(test)]
mod tests {
    #[test]
    fn folds_to_the_configured_number_of_lines() {
        let code = "x([[a\nb\nc]])";
        for (keep, want) in [
            (0, "x([[…]])"),
            (1, "x([[a…]])"),
            (2, "x([[a\nb…]])"),
            (3, code),
        ] {
            assert_eq!(super::fold_multiline_strings(code, keep), want);
        }
        let padded = "x([[\n  a\n  b\n]])";
        assert_eq!(super::fold_multiline_strings(padded, 5), "x([[a\n  b]])");
    }

    #[test]
    fn folds_multiline_long_strings_only() {
        let keep = "-- [[not folded\nx = [[one line]]\ny = \"a\\\"b\"";
        for (code, want) in [
            (
                "write(\"a.txt\", [[line 1\nline 2\nline 3]])",
                "write(\"a.txt\", [[line 1…]])",
            ),
            (
                "write(\"a\", [==[\n  line 1\n  line 2\n]==])",
                "write(\"a\", [==[line 1…]==])",
            ),
            ("write(\"a\", [[open\nrest", "write(\"a\", [[open…"),
            (keep, keep),
        ] {
            assert_eq!(super::fold_multiline_strings(code, 1), want);
        }
    }
}
