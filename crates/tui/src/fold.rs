use crate::style::MORE;

/// Collapses multiline triple-quoted Python strings to a single line
/// while preserving surrounding code, comments and single-line strings.
pub fn fold_multiline_strings(code: &str) -> String {
    let c: Vec<char> = code.chars().collect();
    let mut out = String::with_capacity(code.len());
    let mut i = 0;
    while i < c.len() {
        let ch = c[i];
        if ch == '#' {
            while i < c.len() && c[i] != '\n' {
                out.push(c[i]);
                i += 1;
            }
            continue;
        }
        if ch == '"' || ch == '\'' {
            let triple = c.get(i + 1) == Some(&ch) && c.get(i + 2) == Some(&ch);
            if !triple {
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
            let quote = [ch, ch, ch];
            let body_start = i + 3;
            let mut j = body_start;
            let mut closed = false;
            while j < c.len() {
                if c[j] == '\\' {
                    j = (j + 2).min(c.len());
                    continue;
                }
                if c[j..].starts_with(&quote) {
                    closed = true;
                    break;
                }
                j += 1;
            }
            let body: String = c[body_start..j].iter().collect();
            for _ in 0..3 {
                out.push(ch);
            }
            if body.contains('\n') {
                let mut lines = body.split('\n');
                let head = lines.next().unwrap_or("");
                let preview = if !head.trim().is_empty() {
                    head.trim_end()
                } else {
                    lines.find(|l| !l.trim().is_empty()).map_or("", str::trim)
                };
                out.push_str(preview);
                out.push_str(MORE);
            } else {
                out.push_str(&body);
            }
            if closed {
                for _ in 0..3 {
                    out.push(ch);
                }
                i = j + 3;
            } else {
                i = j;
            }
            continue;
        }
        out.push(ch);
        i += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn folds_multiline_strings_only() {
        let s = "# \"\"\"not a string\nx = '''one line'''\ny = \"a\\\"b\"";
        for (code, want) in [
            (
                "write(\"a.txt\", \"\"\"line 1\nline 2\nline 3\"\"\")",
                "write(\"a.txt\", \"\"\"line 1…\"\"\")",
            ),
            (
                "write(\"a.txt\", \"\"\"\n  line 1\n  line 2\n\"\"\")",
                "write(\"a.txt\", \"\"\"line 1…\"\"\")",
            ),
            (
                "edit(\"a\", \"\"\"o 1\no 2\"\"\", \"\"\"n 1\nn 2\"\"\")",
                "edit(\"a\", \"\"\"o 1…\"\"\", \"\"\"n 1…\"\"\")",
            ),
            (
                "write(\"a.txt\", \"\"\"first\nsecond",
                "write(\"a.txt\", \"\"\"first…",
            ),
            (s, s),
        ] {
            assert_eq!(super::fold_multiline_strings(code), want, "{code:?}");
        }
    }
}
