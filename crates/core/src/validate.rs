//! Strict validation of user config tables and plugin tables before merge/apply.
//! Every diagnostic includes the source (file path or plugin name) and the full key path.

use tuse_config::{check_command_name, check_open_ext};
use tuse_lua::mlua::{Table, Value};

const TOP_KEYS: &[&str] = &[
    "settings",
    "theme",
    "systems",
    "models",
    "prompts",
    "functions",
    "hooks",
    "commands",
    "open",
    "plugins",
];

pub(crate) struct Check<'a> {
    pub(crate) source: &'a str,
    pub(crate) errs: Vec<String>,
}

impl Check<'_> {
    pub(crate) fn err(&mut self, path: &str, msg: impl std::fmt::Display) {
        self.errs.push(format!("{}: {path}: {msg}", self.source));
    }
    pub(crate) fn table(&mut self, path: &str, v: Value) -> Option<Table> {
        match v {
            Value::Table(t) => Some(t),
            other => {
                self.err(path, format!("expected table, got {}", kind(&other)));
                None
            }
        }
    }
    pub(crate) fn string(&mut self, path: &str, v: &Value) -> Option<String> {
        match v {
            Value::String(s) => Some(s.to_string_lossy()),
            other => {
                self.err(path, format!("expected string, got {}", kind(other)));
                None
            }
        }
    }
    pub(crate) fn int(&mut self, path: &str, v: &Value, min: i64, max: i64) -> Option<i64> {
        let Value::Integer(n) = v else {
            self.err(path, format!("expected integer, got {}", kind(v)));
            return None;
        };
        if *n < min || *n > max {
            self.err(path, format!("value {n} out of range ({min}..={max})"));
            return None;
        }
        Some(*n)
    }
    pub(crate) fn array(&mut self, path: &str, t: &Table) -> Vec<(usize, Value)> {
        let len = t.raw_len();
        let mut bad = false;
        for pair in t.pairs::<Value, Value>().flatten() {
            match pair.0 {
                Value::Integer(i) if i >= 1 && (i as usize) <= len => {}
                other => {
                    self.err(
                        path,
                        format!(
                            "expected list (1..={len}), unexpected key {}",
                            key_str(&other)
                        ),
                    );
                    bad = true;
                }
            }
        }
        if bad {
            return vec![];
        }
        (1..=len)
            .filter_map(|i| t.raw_get::<Value>(i).ok().map(|v| (i, v)))
            .collect()
    }
}

pub(crate) fn table(t: &Table, source: &str, is_plugin: bool) -> Result<(), String> {
    let mut c = Check {
        source,
        errs: vec![],
    };
    for pair in t.pairs::<Value, Value>() {
        let (k, v) = pair.map_err(|e| format!("{source}: {e}"))?;
        let Some(key) = as_str_key(&k) else {
            c.err(&key_str(&k), "top-level key must be a string");
            continue;
        };
        match key.as_str() {
            "include" => c.err("include", "was removed; use `require(...)` instead"),
            "model" | "provider" | "effort" => c.err(
                &key,
                "was removed from tuse.lua; use `/models` (selection is stored in SQLite)",
            ),
            "models" => {
                if let Some(sub) = c.table("models", v) {
                    crate::validate_models::check(&mut c, &sub);
                }
            }
            "plugins" if is_plugin => c.err("plugins", "is not allowed in a plugin"),
            "plugins" => {} // validated by `plugins::entries`
            "settings" => {
                if let Some(sub) = c.table("settings", v) {
                    check_settings(&mut c, &sub);
                }
            }
            "theme" => {
                if let Some(sub) = c.table("theme", v) {
                    crate::validate_entries::check_theme(&mut c, &sub);
                }
            }
            "systems" => {
                if let Some(sub) = c.table("systems", v) {
                    for (i, item) in c.array("systems", &sub) {
                        c.string(&format!("systems[{i}]"), &item);
                    }
                }
            }
            "prompts" => {
                if let Some(sub) = c.table("prompts", v) {
                    for (pk, pv) in sub.pairs::<Value, Value>().flatten() {
                        match as_str_key(&pk) {
                            Some(name) => {
                                c.string(&format!("prompts.{name}"), &pv);
                            }
                            None => c.err(
                                &format!("prompts[{}]", key_str(&pk)),
                                "key must be a string",
                            ),
                        }
                    }
                }
            }
            "functions" => {
                if let Some(sub) = c.table("functions", v) {
                    crate::validate_entries::check_functions(&mut c, &sub);
                }
            }
            "hooks" => {
                if let Some(sub) = c.table("hooks", v) {
                    crate::validate_entries::check_hooks(&mut c, &sub);
                }
            }
            "commands" => {
                if let Some(sub) = c.table("commands", v) {
                    crate::validate_entries::check_fn_map(
                        &mut c,
                        "commands",
                        &sub,
                        check_command_name,
                    );
                }
            }
            "open" => {
                if let Some(sub) = c.table("open", v) {
                    crate::validate_entries::check_fn_map(&mut c, "open", &sub, check_open_ext);
                }
            }
            other if !TOP_KEYS.contains(&other) => c.err(other, "unknown key"),
            _ => {}
        }
    }
    if c.errs.is_empty() {
        Ok(())
    } else {
        c.errs.sort();
        Err(c.errs.join("\n"))
    }
}

fn check_settings(c: &mut Check<'_>, t: &Table) {
    for (k, v) in t.pairs::<Value, Value>().flatten() {
        let Some(key) = as_str_key(&k) else {
            c.err(
                &format!("settings[{}]", key_str(&k)),
                "key must be a string",
            );
            continue;
        };
        let p = format!("settings.{key}");
        match key.as_str() {
            "compact_at" => {
                c.int(&p, &v, 0, i64::MAX);
            }
            "max_image_bytes" => {
                c.int(&p, &v, 1, i64::MAX);
            }
            "retries" => {
                c.int(&p, &v, 0, 100);
            }
            "ui" => {
                if let Some(ui) = c.table(&p, v) {
                    crate::validate_entries::check_ui(c, &ui);
                }
            }
            _ => c.err(&p, "unknown key"),
        }
    }
}

pub(crate) fn as_str_key(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.to_string_lossy()),
        _ => None,
    }
}

pub(crate) fn key_str(v: &Value) -> String {
    match v {
        Value::String(s) => s.to_string_lossy(),
        Value::Integer(n) => n.to_string(),
        Value::Number(n) => n.to_string(),
        other => format!("<{}>", kind(other)),
    }
}

pub(crate) fn kind(v: &Value) -> &'static str {
    match v {
        Value::Nil => "nil",
        Value::Boolean(_) => "boolean",
        Value::Integer(_) => "integer",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Table(_) => "table",
        Value::Function(_) => "function",
        _ => "userdata",
    }
}
