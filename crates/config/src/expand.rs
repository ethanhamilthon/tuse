use crate::{Mode, mode_matches};
use serde_yaml::Value;
use std::process::Command;

/// Checks every per-entry `modes` list before expansion so unknown mode names fail
/// before any `#{{cmd}}` runs.
pub fn validate_modes(root: &Value) -> Result<(), String> {
    let Some(map) = root.as_mapping() else {
        return Ok(());
    };
    for key in ["connect", "systems"] {
        if let Some(Value::Sequence(seq)) = map.get(key) {
            for (i, item) in seq.iter().enumerate() {
                check_modes(item).map_err(|e| format!("{key}[{i}].modes: {e}"))?;
            }
        }
    }
    for key in ["prompts", "functions", "hooks"] {
        if let Some(Value::Mapping(entries)) = map.get(key) {
            for (name, item) in entries {
                let label = name.as_str().unwrap_or("?");
                check_modes(item).map_err(|e| format!("{key}.{label}.modes: {e}"))?;
            }
        }
    }
    Ok(())
}

fn check_modes(item: &Value) -> Result<(), String> {
    let Some(modes) = item.as_mapping().and_then(|m| m.get("modes")) else {
        return Ok(());
    };
    serde_yaml::from_value::<Vec<Mode>>(modes.clone())
        .map(|_| ())
        .map_err(|e| e.to_string())
}

fn entry_active(item: &Value, mode: Mode) -> bool {
    let Some(modes) = item.as_mapping().and_then(|m| m.get("modes")) else {
        return true;
    };
    serde_yaml::from_value::<Vec<Mode>>(modes.clone())
        .map(|m| mode_matches(&m, mode))
        .unwrap_or(true)
}

/// Expands every string value in place for `mode`, skipping inactive mode-filtered entries
/// and the Python code in `hooks.<name>.run` and `functions.<name>.run`.
pub fn walk(v: &mut Value, mode: Mode) {
    walk_at(v, &mut vec![], mode);
}

fn walk_at(v: &mut Value, path: &mut Vec<String>, mode: Mode) {
    match v {
        Value::String(s) if !is_code(path) => *s = expand(s),
        Value::Sequence(seq) => {
            let check = matches!(path.as_slice(), [top] if top == "connect" || top == "systems");
            for item in seq.iter_mut() {
                if check && !entry_active(item, mode) {
                    continue;
                }
                walk_at(item, path, mode);
            }
        }
        Value::Mapping(m) => {
            let check = matches!(
                path.as_slice(),
                [top] if top == "prompts" || top == "functions" || top == "hooks"
            );
            for (k, v) in m.iter_mut() {
                if check && !entry_active(v, mode) {
                    continue;
                }
                path.push(k.as_str().unwrap_or_default().to_string());
                walk_at(v, path, mode);
                path.pop();
            }
        }
        Value::Tagged(t) => walk_at(&mut t.value, path, mode),
        _ => {}
    }
}

fn is_code(path: &[String]) -> bool {
    matches!(path, [top, _, run] if (top == "hooks" || top == "functions") && run == "run")
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

#[cfg(test)]
mod tests {
    use crate::{Mode, load};

    #[test]
    fn inactive_entries_do_not_run_shell_commands() {
        let d = std::env::temp_dir().join(format!("tuse-cfg-noexec-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let flag = |n: &str| d.join(format!("ran-{n}"));
        let touch = |n: &str| format!("#{{{{touch '{}'; echo {n}}}}}", flag(n).display());
        std::fs::write(
            d.join("inc.yaml"),
            format!("systems: [\"{}\"]\n", touch("inc")),
        )
        .unwrap();
        let p = d.join("main.yaml");
        std::fs::write(
            &p,
            format!(
                r#"include:
  - {{file: inc.yaml, modes: [tui]}}
connect:
  - {{name: gw, base_url: http://a, api_key: "{}", modes: [tui]}}
systems:
  - "{}"
  - {{text: "{}", modes: [tui]}}
prompts:
  p: {{text: "{}", modes: [tui]}}
functions:
  f: {{description: "{}", run: "return 1", modes: [tui]}}
hooks:
  h: {{events: ["{}"], run: "pass", modes: [tui]}}
"#,
                touch("conn"),
                touch("always"),
                touch("sys"),
                touch("prompt"),
                touch("fn"),
                touch("hook"),
            ),
        )
        .unwrap();

        let c = load(Some(&p), Mode::Json).unwrap().config;
        assert_eq!(c.systems[0].text(), "always");
        assert!(flag("always").exists());
        for n in ["inc", "conn", "sys", "prompt", "fn", "hook"] {
            assert!(!flag(n).exists(), "command `{n}` ran outside its mode");
        }

        let c = load(Some(&p), Mode::Tui).unwrap().config;
        assert_eq!(c.connect[0].api_key, "conn");
        assert_eq!(c.prompts["p"].text(), "prompt");
        assert_eq!(c.functions["f"].description, "fn");
        for n in ["inc", "conn", "sys", "prompt", "fn", "hook"] {
            assert!(flag(n).exists(), "command `{n}` did not run in tui");
        }

        let bad_flag = flag("bad");
        std::fs::write(
            &p,
            format!(
                "systems:\n  - \"#{{{{touch '{}'}}}}\"\n  - {{text: x, modes: [bogus]}}\n",
                bad_flag.display()
            ),
        )
        .unwrap();
        assert!(load(Some(&p), Mode::Tui).is_err());
        assert!(!bad_flag.exists());
        std::fs::remove_dir_all(d).ok();
    }
}
