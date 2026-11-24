use crate::kit::style::MORE;

pub fn fold_multiline_strings(code: &str) -> String {
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
            let mut lines = body.split('\n');
            let head = lines.next().unwrap_or("");
            let preview = if head.trim().is_empty() {
                lines.find(|l| !l.trim().is_empty()).map_or("", str::trim)
            } else {
                head.trim_end()
            };
            out.push_str(preview);
            out.push_str(MORE);
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
            assert_eq!(super::fold_multiline_strings(code), want);
        }
    }
}
