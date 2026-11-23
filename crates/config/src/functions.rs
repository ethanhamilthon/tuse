use crate::Config;

const RESERVED: &str = "and break do else elseif end false for function goto if in local nil not or repeat return then true until while read write append delete edit grep sh sleep spawn task_get task_stop task_list docs set_title session show_markdown show_picker show_question print json inspect";

pub fn check_function_name(name: &str) -> Option<&'static str> {
    let mut chars = name.chars();
    if !chars.next().is_some_and(|c| c == '_' || c.is_alphabetic())
        || !chars.all(|c| c == '_' || c.is_alphanumeric())
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

pub fn validate(config: &Config) -> Result<(), String> {
    for (name, f) in &config.functions {
        if let Some(reason) = check_function_name(name) {
            return Err(format!("functions.{name}: {reason}"));
        }
        if f.description.trim().is_empty() {
            return Err(format!("functions.{name}: description must not be empty"));
        }
    }
    for name in &config.commands {
        if let Some(reason) = check_command_name(name) {
            return Err(format!("commands.{name}: {reason}"));
        }
    }
    for ext in &config.open {
        if let Some(reason) = check_open_ext(ext) {
            return Err(format!("open.{ext}: {reason}"));
        }
    }
    Ok(())
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
            let mut cfg = Config::default();
            cfg.functions.insert(
                name.into(),
                crate::Function {
                    args: String::new(),
                    description: description.into(),
                    ..Default::default()
                },
            );
            assert!(
                validate(&cfg)
                    .unwrap_err()
                    .contains(&format!("functions.{name}"))
            );
        }
        assert!(check_open_ext(".md").is_some());
        assert!(check_open_ext("md").is_none());
        assert!(check_command_name("session").is_some());
    }
}
