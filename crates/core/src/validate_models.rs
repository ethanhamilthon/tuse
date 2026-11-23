use crate::validate::{Check, key_str};
use tuse_lua::mlua::{Table, Value};

pub(crate) fn check(c: &mut Check<'_>, models: &Table) {
    for (i, item) in c.array("models", models) {
        let path = format!("models[{i}]");
        let Some(model) = c.table(&path, item) else {
            continue;
        };
        let name = model.raw_get::<Value>("name").unwrap_or(Value::Nil);
        let name_path = format!("{path}.name");
        if let Some(name) = c.string(&name_path, &name) {
            let (provider, id) = tuse_config::split_model(&name);
            if provider.is_empty() || id.is_empty() || name.chars().any(char::is_whitespace) {
                c.err(&name_path, "expected a provider/model name");
            }
        }
        for (k, v) in model.pairs::<Value, Value>().flatten() {
            let key = key_str(&k);
            let p = format!("{path}.{key}");
            match key.as_str() {
                "name" => {}
                "display" => {
                    c.string(&p, &v);
                }
                "window" | "compact_at" => {
                    c.int(&p, &v, 0, i64::MAX);
                }
                "prices" => {
                    if let Some(prices) = c.table(&p, v) {
                        for (k, v) in prices.pairs::<Value, Value>().flatten() {
                            let key = key_str(&k);
                            let price = format!("{p}.{key}");
                            match key.as_str() {
                                "input" | "in" | "output" | "out" | "cache_read" | "cache"
                                | "cache_write" => match v {
                                    Value::Integer(n) if n >= 0 => {}
                                    Value::Number(n) if n.is_finite() && n >= 0.0 => {}
                                    _ => c.err(&price, "expected a finite non-negative number"),
                                },
                                _ => c.err(&price, "unknown key"),
                            }
                        }
                        for (canonical, alias) in
                            [("input", "in"), ("output", "out"), ("cache_read", "cache")]
                        {
                            if prices.contains_key(canonical).unwrap_or(false)
                                && prices.contains_key(alias).unwrap_or(false)
                            {
                                c.err(&format!("{p}.{alias}"), format!("duplicates `{canonical}`"));
                            }
                        }
                    }
                }
                _ => c.err(&p, "unknown key"),
            }
        }
    }
}
