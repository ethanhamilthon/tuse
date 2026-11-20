use std::collections::BTreeMap;

/// Replaces every `#name` token with the text of prompt `name`.
pub fn expand_prompts(text: &str, prompts: &BTreeMap<String, String>) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(i) = rest.find('#') {
        let at_start = i == 0 || rest[..i].ends_with(char::is_whitespace);
        let name: String = rest[i + 1..]
            .chars()
            .take_while(|c| c.is_alphanumeric() || matches!(c, '-' | '_' | '.'))
            .collect();
        let name = name.trim_end_matches('.');
        out.push_str(&rest[..i]);
        match prompts.get(name) {
            Some(p) if at_start && !name.is_empty() => {
                out.push_str(p.trim_end());
                rest = &rest[i + 1 + name.len()..];
            }
            _ => {
                out.push('#');
                rest = &rest[i + 1..];
            }
        }
    }
    out.push_str(rest);
    out
}
