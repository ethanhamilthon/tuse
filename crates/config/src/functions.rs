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

const RESERVED: &str = "and break do else elseif end false for function goto if in local nil not or repeat return then true until while _G _VERSION assert collectgarbage dofile error getmetatable ipairs load loadfile next pairs pcall print rawequal rawget rawlen rawset require select setmetatable tonumber tostring type warn xpcall coroutine io math os package string table utf8";

pub fn check_function_name(name: &str) -> Option<&'static str> {
    let mut chars = name.chars();
    if !chars
        .next()
        .is_some_and(|c| c == '_' || c.is_ascii_alphabetic())
        || !chars.all(|c| c == '_' || c.is_ascii_alphanumeric())
    {
        Some("invalid Lua name")
    } else if RESERVED.split_whitespace().any(|r| r == name) {
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
    fn validates_function_command_and_open_names() {
        for bad in [
            "not-valid",
            "1abc",
            "end",
            "os",
            "package",
            "require",
            "print",
        ] {
            let v = serde_json::json!({"functions": {bad: "custom"}});
            let err = crate::from_value(v, "t").unwrap_err();
            assert!(err.starts_with("t: functions"), "{bad}: {err}");
        }
        for ok in ["websearch", "read", "sh", "set_title", "session"] {
            assert!(check_function_name(ok).is_none(), "{ok}");
        }
        assert!(
            crate::from_value(serde_json::json!({"functions": {"json": "override"}}), "t").is_err()
        );
        assert!(
            crate::from_value(serde_json::json!({"functions": {"json": "disabled"}}), "t").is_ok()
        );
        assert!(check_open_ext(".md").is_some());
        assert!(check_open_ext("md").is_none());
        assert!(check_command_name("session").is_some());
    }
}
