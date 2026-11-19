use serde_yaml::{Mapping, Value};

/// Flattens `extend` in the `themes` mapping: every theme becomes its base
/// theme's keys overridden by its own. The base is another entry of `themes`
/// or `default` (the built-in theme).
pub fn resolve(root: &mut Value) -> Result<(), String> {
    let Some(themes) = root.get("themes").and_then(Value::as_mapping).cloned() else {
        return Ok(());
    };
    let mut out = Mapping::new();
    for (name, _) in &themes {
        let flat = flatten(name.as_str().unwrap_or(""), &themes, 0)?;
        out.insert(name.clone(), Value::Mapping(flat));
    }
    root["themes"] = Value::Mapping(out);
    Ok(())
}

fn flatten(name: &str, themes: &Mapping, depth: usize) -> Result<Mapping, String> {
    if depth > 16 {
        return Err(format!("theme {name}: extend loop"));
    }
    let own = match themes.get(name) {
        Some(Value::Mapping(m)) => m.clone(),
        Some(Value::Null) => Mapping::new(),
        Some(_) => return Err(format!("theme {name}: must be a mapping")),
        None if name == "default" => return Ok(Mapping::new()),
        None => return Err(format!("theme {name} not found")),
    };
    let mut flat = match own.get("extend").and_then(Value::as_str) {
        Some(base) => flatten(base, themes, depth + 1)?,
        None => Mapping::new(),
    };
    for (k, v) in own {
        if k.as_str() != Some("extend") {
            flat.insert(k, v);
        }
    }
    Ok(flat)
}
