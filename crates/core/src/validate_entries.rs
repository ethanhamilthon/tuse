//! Validation of `functions`, `hooks`, `commands`, and `open` tables.

use crate::validate::{Check, as_str_key, key_str, kind};
use tuse_config::{HOOK_EVENTS, check_function_name};
use tuse_lua::mlua::{Table, Value};

pub(crate) fn check_fn_map(
    c: &mut Check<'_>,
    base: &str,
    t: &Table,
    check_name: fn(&str) -> Option<&'static str>,
) {
    for (k, v) in t.pairs::<Value, Value>().flatten() {
        let Some(key) = as_str_key(&k) else {
            c.err(&format!("{base}[{}]", key_str(&k)), "key must be a string");
            continue;
        };
        let p = format!("{base}.{key}");
        if let Some(reason) = check_name(&key) {
            c.err(&p, reason);
        }
        if !matches!(v, Value::Function(_)) {
            c.err(&p, format!("expected function, got {}", kind(&v)));
        }
    }
}

pub(crate) fn check_functions(c: &mut Check<'_>, t: &Table) {
    for (k, v) in t.pairs::<Value, Value>().flatten() {
        let Some(name) = as_str_key(&k) else {
            c.err(
                &format!("functions[{}]", key_str(&k)),
                "key must be a string",
            );
            continue;
        };
        let base = format!("functions.{name}");
        if let Some(reason) = check_function_name(&name) {
            c.err(&base, reason);
        }
        let Some(f) = c.table(&base, v) else { continue };
        let (mut has_run, mut has_desc) = (false, false);
        for (fk, fv) in f.pairs::<Value, Value>().flatten() {
            let Some(key) = as_str_key(&fk) else {
                c.err(&format!("{base}[{}]", key_str(&fk)), "key must be a string");
                continue;
            };
            let p = format!("{base}.{key}");
            match key.as_str() {
                "modes" => c.err(
                    &p,
                    "was removed; branch on require(\"tuse\").session().mode",
                ),
                "run" => {
                    has_run = true;
                    if !matches!(fv, Value::Function(_)) {
                        c.err(&p, format!("expected function, got {}", kind(&fv)));
                    }
                }
                "description" => {
                    has_desc = true;
                    if let Some(s) = c.string(&p, &fv)
                        && s.trim().is_empty()
                    {
                        c.err(&p, "must not be empty");
                    }
                }
                "args" => {
                    c.string(&p, &fv);
                }
                "show_agent" => {
                    if !matches!(fv, Value::Boolean(_)) {
                        c.err(&p, format!("expected boolean, got {}", kind(&fv)));
                    }
                }
                _ => c.err(&p, "unknown key"),
            }
        }
        if !has_run {
            c.err(&format!("{base}.run"), "is required");
        }
        if !has_desc {
            c.err(&format!("{base}.description"), "is required");
        }
    }
}

pub(crate) fn check_hooks(c: &mut Check<'_>, t: &Table) {
    for (k, v) in t.pairs::<Value, Value>().flatten() {
        let Some(name) = as_str_key(&k) else {
            c.err(&format!("hooks[{}]", key_str(&k)), "key must be a string");
            continue;
        };
        let base = format!("hooks.{name}");
        let Some(h) = c.table(&base, v) else { continue };
        let (mut has_run, mut has_events) = (false, false);
        for (hk, hv) in h.pairs::<Value, Value>().flatten() {
            let Some(key) = as_str_key(&hk) else {
                c.err(&format!("{base}[{}]", key_str(&hk)), "key must be a string");
                continue;
            };
            let p = format!("{base}.{key}");
            match key.as_str() {
                "modes" => c.err(
                    &p,
                    "was removed; branch on require(\"tuse\").session().mode",
                ),
                "run" => {
                    has_run = true;
                    if !matches!(hv, Value::Function(_)) {
                        c.err(&p, format!("expected function, got {}", kind(&hv)));
                    }
                }
                "events" => {
                    has_events = true;
                    if let Some(list) = c.table(&p, hv) {
                        let items = c.array(&p, &list);
                        if items.is_empty() && list.raw_len() == 0 {
                            c.err(&p, "must contain at least one event");
                        }
                        for (i, item) in items {
                            let ep = format!("{p}[{i}]");
                            if let Some(ev) = c.string(&ep, &item) {
                                if ev == "process_start" {
                                    c.err(&ep, "event `process_start` was renamed to `start`");
                                } else if ev == "process_end" {
                                    c.err(&ep, "event `process_end` was renamed to `close`");
                                } else if !HOOK_EVENTS.contains(&ev.as_str()) {
                                    c.err(
                                        &ep,
                                        format!(
                                            "unknown event `{ev}`; events are {}",
                                            HOOK_EVENTS.join(", ")
                                        ),
                                    );
                                }
                            }
                        }
                    }
                }
                _ => c.err(&p, "unknown key"),
            }
        }
        if !has_events {
            c.err(&format!("{base}.events"), "is required");
        }
        if !has_run {
            c.err(&format!("{base}.run"), "is required");
        }
    }
}
const THEME_KEYS: &[&str] = &[
    "bg", "surface", "fg", "dim", "muted", "border", "accent", "ok", "err", "info",
];

pub(crate) fn check_theme(c: &mut Check<'_>, t: &Table) {
    for (k, v) in t.pairs::<Value, Value>().flatten() {
        let Some(key) = as_str_key(&k) else {
            c.err(&format!("theme[{}]", key_str(&k)), "key must be a string");
            continue;
        };
        let p = format!("theme.{key}");
        if !THEME_KEYS.contains(&key.as_str()) {
            c.err(&p, "unknown color token");
        } else if let Some(s) = c.string(&p, &v)
            && !tuse_config::valid_color(&s)
        {
            c.err(&p, format!("invalid color `{s}`"));
        }
    }
}
pub(crate) fn check_ui(c: &mut Check<'_>, t: &Table) {
    for (k, v) in t.pairs::<Value, Value>().flatten() {
        let Some(key) = as_str_key(&k) else {
            c.err(
                &format!("settings.ui[{}]", key_str(&k)),
                "key must be a string",
            );
            continue;
        };
        let p = format!("settings.ui.{key}");
        match key.as_str() {
            "fold_strings" => {
                if !matches!(v, Value::Boolean(_)) {
                    c.err(&p, format!("expected boolean, got {}", kind(&v)));
                }
            }
            "tool_line" | "diff_line" | "output_line" | "reasoning_line" => {
                c.int(&p, &v, -1, i32::MAX as i64);
            }
            "input_max_height" => {
                if c.int(&p, &v, -1, i32::MAX as i64) == Some(0) {
                    c.err(&p, "must be -1 or >= 1");
                }
            }
            "scroll_step" => {
                c.int(&p, &v, 1, u16::MAX as i64);
            }
            "pad_x" | "message_gap" => {
                c.int(&p, &v, 0, u16::MAX as i64);
            }
            "list_max_height" => {
                c.int(&p, &v, 1, i32::MAX as i64);
            }
            _ => c.err(&p, "unknown key"),
        }
    }
}
