use crate::Config;

pub fn validate(config: &Config) -> Result<(), String> {
    const RESERVED: &str = "False None True and as assert async await break class continue def del elif else except finally for from global if import in is lambda nonlocal not or pass raise return try while with yield read write edit grep sh task_run task_add task_get task_stop task_list docs set_title";
    for (name, f) in &config.functions {
        let mut chars = name.chars();
        let reason = if !chars.next().is_some_and(|c| c == '_' || c.is_alphabetic())
            || !chars.all(|c| c == '_' || c.is_alphanumeric())
        {
            "invalid Python identifier"
        } else if RESERVED.split_whitespace().any(|r| r == name) {
            "reserved name"
        } else if f.description.trim().is_empty() || f.run.trim().is_empty() {
            "description and run must not be empty"
        } else {
            continue;
        };
        return Err(format!("functions.{name}: {reason}"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_invalid_definitions() {
        for (name, description, run) in [
            ("not-valid", "a", "pass"),
            ("def", "a", "pass"),
            ("read", "a", "pass"),
            ("set_title", "a", "pass"),
            ("foo", "", "pass"),
            ("foo", "a", "  "),
        ] {
            let mut cfg = Config::default();
            cfg.functions.insert(
                name.into(),
                crate::Function {
                    args: String::new(),
                    description: description.into(),
                    run: run.into(),
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
