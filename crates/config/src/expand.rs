use serde_yaml::Value;
use std::process::Command;

/// Expands every string value in place.
pub fn walk(v: &mut Value) {
    match v {
        Value::String(s) => *s = expand(s),
        Value::Sequence(seq) => seq.iter_mut().for_each(walk),
        Value::Mapping(m) => m.iter_mut().for_each(|(_, v)| walk(v)),
        Value::Tagged(t) => walk(&mut t.value),
        _ => {}
    }
}

/// `#{{cmd}}` becomes the trimmed output of `bash -c cmd`; `$VAR` and `${VAR}`
/// become the environment value (empty when unset); `$$` is a literal `$`.
pub fn expand(s: &str) -> String {
    let s = commands(s);
    if !s.contains('$') {
        return s;
    }
    vars(&s, |name| std::env::var(name).unwrap_or_default())
}

fn commands(s: &str) -> String {
    let mut out = String::new();
    let mut rest = s;
    while let Some(start) = rest.find("#{{") {
        let Some(len) = rest[start + 3..].find("}}") else {
            break;
        };
        out.push_str(&rest[..start]);
        out.push_str(&run(&rest[start + 3..start + 3 + len]));
        rest = &rest[start + 3 + len + 2..];
    }
    out.push_str(rest);
    out
}

fn run(cmd: &str) -> String {
    match Command::new("bash").arg("-c").arg(cmd).output() {
        Ok(o) => String::from_utf8_lossy(&o.stdout)
            .trim_end_matches(['\n', '\r'])
            .to_string(),
        Err(_) => String::new(),
    }
}

pub fn vars(s: &str, get: impl Fn(&str) -> String) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] != '$' {
            out.push(chars[i]);
            i += 1;
            continue;
        }
        if chars.get(i + 1) == Some(&'$') {
            out.push('$');
            i += 2;
            continue;
        }
        if chars.get(i + 1) == Some(&'{')
            && let Some(end) = chars[i + 2..].iter().position(|&c| c == '}')
        {
            let name: String = chars[i + 2..i + 2 + end].iter().collect();
            out.push_str(&get(&name));
            i += end + 3;
            continue;
        }
        let name: String = chars[i + 1..]
            .iter()
            .take_while(|c| c.is_ascii_alphanumeric() || **c == '_')
            .collect();
        if name.is_empty() || name.starts_with(|c: char| c.is_ascii_digit()) {
            out.push('$');
            i += 1;
        } else {
            out.push_str(&get(&name));
            i += 1 + name.len();
        }
    }
    out
}
