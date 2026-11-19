use monty_types::MontyObject;

/// What a snippet's last expression shows: nothing for `None`, text as it is,
/// a `sh` result as its output plus an exit note, anything else as its `repr`.
pub fn value(v: &MontyObject) -> String {
    let r = v.as_ref();
    match r.type_name() {
        "NoneType" => String::new(),
        "str" => r.as_str().unwrap_or_default().to_string(),
        _ => match sh_result(v) {
            Some(s) => s,
            None => r.py_repr(),
        },
    }
}

/// `out`, then `[exit code N]` / `[timed out]`, of a result built by `sh`.
fn sh_result(v: &MontyObject) -> Option<String> {
    let items = v.as_ref().items()?;
    let repr = v.py_repr();
    if !repr.starts_with("Sh(out=") || items.len() != 4 {
        return None;
    }
    let mut out = items[0].as_str()?.to_string();
    let note = if items[3].as_bool() == Some(true) {
        "[timed out]".to_string()
    } else {
        match items[1].as_int() {
            Some(0) => String::new(),
            Some(c) => format!("[exit code {c}]"),
            None => "[killed by signal]".to_string(),
        }
    };
    if !note.is_empty() {
        if !out.is_empty() && !out.ends_with('\n') {
            out.push('\n');
        }
        out.push_str(&note);
    }
    Some(out)
}
