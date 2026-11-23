use crate::Config;

pub fn validate(config: &Config) -> Result<(), String> {
    const RESERVED: &str = "and break do else elseif end false for function goto if in local nil not or repeat return then true until while read write append delete edit grep sh sleep spawn task_get task_stop task_list docs set_title show_markdown show_picker show_question print json inspect";
    for (name, f) in &config.functions {
        let mut chars = name.chars();
        let reason = if !chars.next().is_some_and(|c| c == '_' || c.is_alphabetic())
            || !chars.all(|c| c == '_' || c.is_alphanumeric())
        {
            "invalid Lua name"
        } else if RESERVED.split_whitespace().any(|r| r == name) {
            "reserved name"
        } else if f.description.trim().is_empty() {
            "description must not be empty"
        } else {
            continue;
        };
        return Err(format!("functions.{name}: {reason}"));
    }
    for name in &config.commands {
        let reason = if name.is_empty()
            || name.starts_with('/')
            || name.chars().any(|c| c.is_whitespace() || c.is_control())
        {
            "invalid command name"
        } else if crate::BUILTIN_COMMANDS.contains(&name.as_str()) {
            "cannot override built-in command"
        } else {
            continue;
        };
        return Err(format!("commands.{name}: {reason}"));
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
    }
}
