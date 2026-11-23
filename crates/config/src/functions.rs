/// Every Lua host function: globals in the agent's `lua` tool, exported by `require("tuse")`
/// in files, and highlighted by the TUI.
pub const HOST_FUNCTIONS: &[&str] = &[
    "sh",
    "sleep",
    "spawn",
    "read",
    "write",
    "append",
    "delete",
    "edit",
    "grep",
    "docs",
    "set_title",
    "session",
    "task_get",
    "task_list",
    "task_stop",
    "show",
    "pick",
    "ask",
    "reload",
    "llm",
    "json",
    "inspect",
];

const KEYWORDS: &str = "and break do else elseif end false for function goto if in local nil not or repeat return then true until while print";

pub fn check_function_name(name: &str) -> Option<&'static str> {
    let mut chars = name.chars();
    if !chars.next().is_some_and(|c| c == '_' || c.is_alphabetic())
        || !chars.all(|c| c == '_' || c.is_alphanumeric())
    {
        Some("invalid Lua name")
    } else if KEYWORDS.split_whitespace().any(|r| r == name) || HOST_FUNCTIONS.contains(&name) {
        Some("reserved name")
    } else {
        None
    }
}

pub fn check_command_name(name: &str) -> Option<&'static str> {
    if name.is_empty()
        || name.starts_with('/')
        || name.chars().any(|c| c.is_whitespace() || c.is_control())
    {
        Some("invalid command name")
    } else if crate::BUILTIN_COMMANDS.contains(&name) {
        Some("cannot override built-in command")
    } else {
        None
    }
}

pub fn check_open_ext(ext: &str) -> Option<&'static str> {
    if ext.is_empty() {
        Some("extension must not be empty")
    } else if ext.starts_with('.') {
        Some("extension must not start with `.`")
    } else if ext
        .chars()
        .any(|c| c == '/' || c == '\\' || c.is_whitespace() || c.is_control())
    {
        Some("invalid file extension")
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_invalid_definitions() {
        for (name, description) in [
            ("not-valid", "a"),
            ("end", "a"),
            ("read", "a"),
            ("set_title", "a"),
            ("session", "a"),
            ("foo", ""),
        ] {
            let v = serde_json::json!({"functions": {name: {"description": description}}});
            let err = crate::from_value(v, "t").unwrap_err();
            assert!(err.starts_with("t: functions"), "{err}");
        }
        assert!(check_open_ext(".md").is_some());
        assert!(check_open_ext("md").is_none());
        assert!(check_command_name("session").is_some());
    }
}
