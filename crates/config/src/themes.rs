use serde_json::{Map, Value};

/// Flattens `extend` in `themes`: every theme becomes its base theme's keys
/// overridden by its own. The base is another entry of `themes` or `default`.
pub fn resolve(root: &mut Value) -> Result<(), String> {
    let Some(themes) = root.get("themes").and_then(Value::as_object).cloned() else {
        return Ok(());
    };
    let mut out = Map::new();
    for name in themes.keys() {
        out.insert(name.clone(), Value::Object(flatten(name, &themes, 0)?));
    }
    root["themes"] = Value::Object(out);
    Ok(())
}

fn flatten(
    name: &str,
    themes: &Map<String, Value>,
    depth: usize,
) -> Result<Map<String, Value>, String> {
    if depth > 16 {
        return Err(format!("theme {name}: extend loop"));
    }
    let own = match themes.get(name) {
        Some(Value::Object(m)) => m.clone(),
        Some(Value::Null) => Map::new(),
        Some(Value::Array(a)) if a.is_empty() => Map::new(),
        Some(_) => return Err(format!("theme {name}: must be a table")),
        None if name == "default" => return Ok(Map::new()),
        None => return Err(format!("theme {name} not found")),
    };
    let mut flat = match own.get("extend").and_then(Value::as_str) {
        Some(base) => flatten(base, themes, depth + 1)?,
        None => Map::new(),
    };
    for (k, v) in own {
        if k != "extend" {
            flat.insert(k, v);
        }
    }
    Ok(flat)
}
