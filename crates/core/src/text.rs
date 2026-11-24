pub(crate) fn error_line(text: &str) -> String {
    match text.find("\nstack traceback:") {
        Some(i) => last_line(&text[..i]),
        None => last_line(text),
    }
}

pub(crate) fn last_line(text: &str) -> String {
    text.lines()
        .rev()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or("failed")
        .to_string()
}
