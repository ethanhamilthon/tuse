use serde_json::{Map, Value as Json, json};
use std::collections::{BTreeMap, BTreeSet};
use tuse_config::{Config, HOST_FUNCTIONS, InstalledPlugin, from_value};
use tuse_lua::mlua::{Function, Lua, Table, Value};

pub(crate) const FLAT_SECTIONS: [&str; 10] = [
    "ui",
    "history",
    "open",
    "providers",
    "models",
    "systems",
    "prompts",
    "functions",
    "hooks",
    "commands",
];

#[derive(Clone)]
pub(crate) struct Contribution {
    pub namespace: Option<String>,
    pub source: String,
    pub layer: u8,
    pub table: Table,
    pub json: Json,
}

pub(crate) struct PluginSelector {
    pub only: Option<Vec<String>>,
    pub without: Option<Vec<String>>,
    pub opts: BTreeMap<String, Value>,
}

fn as_table<'a>(v: &'a Value, src: &str, path: &str) -> Result<&'a Table, String> {
    match v {
        Value::Table(t) => Ok(t),
        o => Err(format!(
            "{src}: {path}: expected a table, got {}",
            o.type_name()
        )),
    }
}

pub(crate) type Extracted = (Option<Value>, Option<Table>, Vec<(String, Table)>);

pub(crate) fn extract_namespaces(
    t: &Table,
    src: &str,
    allow_global: bool,
    only: Option<&str>,
) -> Result<Extracted, String> {
    let (mut global, mut plugins, mut ns) = (None, None, vec![]);
    for (k, v) in str_entries(t, src, None)? {
        match k.as_str() {
            "global" if allow_global => match v {
                Value::Boolean(false) | Value::Table(_) => global = Some(v),
                o => {
                    return Err(format!(
                        "{src}: global: expected false or a table, got {}",
                        o.type_name()
                    ));
                }
            },
            "global" => return Err(format!("{src}: global: unknown key")),
            "plugins" => plugins = Some(as_table(&v, src, "plugins")?.clone()),
            "settings" => return Err(format!("{src}: settings: moved to ui, history, and open")),
            "theme" => return Err(format!("{src}: theme: moved to ui.theme")),
            s if FLAT_SECTIONS.contains(&s) => {
                return Err(format!(
                    "{src}: {k}: flat top-level section '{k}' is not supported"
                ));
            }
            "" => return Err(format!("{src}: namespace name must not be empty")),
            _ if only.is_none_or(|name| name == k) => {
                ns.push((k.clone(), as_table(&v, src, &k)?.clone()));
            }
            _ => {}
        }
    }
    Ok((global, plugins, ns))
}

pub(crate) fn parse_plugin_selector(
    t: &Table,
    src: &str,
    installed: &[InstalledPlugin],
) -> Result<PluginSelector, String> {
    let known = |n: &str| installed.iter().any(|p| p.name == n);
    let (mut only, mut without, mut opts) = (None, None, BTreeMap::new());
    for (k, v) in str_entries(t, src, Some("plugins"))? {
        match k.as_str() {
            "only" | "without" => {
                let pfx = format!("{src}: plugins.{k}");
                let list = parse_str_list(&v, &pfx, "plugin", false)?;
                if let Some((i, n)) = list.iter().enumerate().find(|(_, n)| !known(n)) {
                    return Err(format!("{pfx}[{}]: unknown plugin `{n}`", i + 1));
                }
                *(if k == "only" { &mut only } else { &mut without }) = Some(list);
            }
            "opts" => {
                for (n, ov) in str_entries(
                    as_table(&v, src, "plugins.opts")?,
                    src,
                    Some("plugins.opts"),
                )? {
                    if !known(&n) {
                        return Err(format!("{src}: plugins.opts.{n}: unknown plugin `{n}`"));
                    }
                    opts.insert(n, ov);
                }
            }
            _ => return Err(format!("{src}: plugins.{k}: unknown key")),
        }
    }
    if only.is_some() && without.is_some() {
        return Err(format!("{src}: plugins: cannot set both only and without"));
    }
    Ok(PluginSelector {
        only,
        without,
        opts,
    })
}

pub(crate) fn select_plugins<'a>(
    installed: &'a [InstalledPlugin],
    sel: Option<&PluginSelector>,
) -> Vec<&'a InstalledPlugin> {
    match sel {
        Some(PluginSelector { only: Some(o), .. }) => {
            installed.iter().filter(|p| o.contains(&p.name)).collect()
        }
        Some(PluginSelector {
            without: Some(w), ..
        }) => installed.iter().filter(|p| !w.contains(&p.name)).collect(),
        _ => installed.iter().collect(),
    }
}

fn lua_index(k: &Value) -> Option<usize> {
    match *k {
        Value::Integer(n) if n >= 1 => Some(n as usize),
        Value::Number(n) if n >= 1.0 && n.fract() == 0.0 && n <= usize::MAX as f64 => {
            Some(n as usize)
        }
        _ => None,
    }
}

fn parse_str_list(v: &Value, pfx: &str, kind: &str, no_ws: bool) -> Result<Vec<String>, String> {
    let Value::Table(_) = v else {
        return Err(format!(
            "{pfx}: expected list of strings, got {}",
            v.type_name()
        ));
    };
    let arr = match to_json(v, pfx, false, None)? {
        Json::Array(a) => a,
        Json::Object(m) if m.is_empty() => vec![],
        _ => return Err(format!("{pfx}: expected a dense list")),
    };
    let mut out = Vec::with_capacity(arr.len());
    for (i, item) in arr.into_iter().enumerate() {
        let Some(s) = item.as_str() else {
            return Err(format!("{pfx}[{}]: expected string", i + 1));
        };
        if no_ws && (s.is_empty() || s.chars().any(char::is_whitespace)) {
            let why = if s.is_empty() {
                "must not be empty"
            } else {
                "must not contain whitespace"
            };
            return Err(format!("{pfx}[{}]: {why}", i + 1));
        }
        if out.iter().any(|x| x == s) {
            return Err(format!("{pfx}[{}]: duplicate {kind} `{s}`", i + 1));
        }
        out.push(s.to_string());
    }
    Ok(out)
}

fn sec_entries(v: &Value, src: &str, sec: &str) -> Result<Vec<(String, Value)>, String> {
    str_entries(as_table(v, src, sec)?, src, Some(sec))
}

pub(crate) fn validate_contribution(
    t: &Table,
    src: &str,
    ns: Option<&str>,
) -> Result<Json, String> {
    let mut obj = Map::new();
    for (k, v) in str_entries(t, src, ns)? {
        let sec = ns.map_or_else(|| k.clone(), |n| format!("{n}.{k}"));
        let pfx = format!("{src}: {sec}");
        let j = match k.as_str() {
            "settings" => return Err(format!("{pfx}: moved to ui, history, and open")),
            "theme" => return Err(format!("{pfx}: moved to ui.theme")),
            s if !FLAT_SECTIONS.contains(&s) => return Err(format!("{pfx}: unknown key")),
            "functions" => {
                let mut m = Map::new();
                for (n, val) in sec_entries(&v, src, &sec)? {
                    let known = HOST_FUNCTIONS.contains(&n.as_str());
                    let kind = match val {
                        Value::Function(_) if known => "override",
                        Value::Function(_) => "custom",
                        Value::Boolean(false)
                            if known || tuse_config::check_function_name(&n).is_some() =>
                        {
                            "disabled"
                        }
                        Value::Boolean(false) => {
                            return Err(format!("{pfx}.{n}: cannot disable unknown function"));
                        }
                        o => {
                            return Err(format!(
                                "{pfx}.{n}: expected function or false, got {}",
                                o.type_name()
                            ));
                        }
                    };
                    m.insert(n, Json::String(kind.into()));
                }
                Json::Object(m)
            }
            "hooks" => {
                let mut m = Map::new();
                for (n, val) in sec_entries(&v, src, &sec)? {
                    let hp = format!("{pfx}.{n}");
                    req_fn(as_table(&val, src, &format!("{sec}.{n}"))?, &hp)?;
                    m.insert(n, to_json(&val, &hp, false, Some("run"))?);
                }
                Json::Object(m)
            }
            "commands" => {
                let mut m = Map::new();
                for (n, val) in sec_entries(&v, src, &sec)? {
                    let cp = format!("{pfx}.{n}");
                    let c = as_table(&val, src, &format!("{sec}.{n}"))?;
                    let desc = match c
                        .raw_get::<Value>("desc")
                        .map_err(|e| format!("{cp}.desc: {e}"))?
                    {
                        Value::Nil => String::new(),
                        Value::String(s) => s.to_string_lossy(),
                        _ => return Err(format!("{cp}.desc: expected a string")),
                    };
                    let (args, run) = val_entry(c, &["args", "desc", "run"], &cp)?;
                    let info = run.info();
                    let params = (!info.is_vararg).then_some(info.num_params as usize);
                    m.insert(n, json!({ "desc": desc, "args": args, "params": params }));
                }
                Json::Object(m)
            }
            "open" => {
                let mut out = vec![];
                for (n, val) in sec_entries(&v, src, &sec)? {
                    if !matches!(val, Value::Function(_)) {
                        return Err(format!(
                            "{pfx}.{n}: expected function, got {}",
                            val.type_name()
                        ));
                    }
                    out.push(Json::String(n));
                }
                Json::Array(out)
            }
            "prompts" => {
                let mut m = Map::new();
                for (n, val) in sec_entries(&v, src, &sec)? {
                    let j = match val {
                        Value::String(s) => Json::String(s.to_string_lossy()),
                        Value::Table(p) => {
                            let (args, _) = val_entry(&p, &["args", "run"], &format!("{pfx}.{n}"))?;
                            json!({ "kind": "function", "args": args })
                        }
                        o => {
                            return Err(format!(
                                "{pfx}.{n}: expected string or table, got {}",
                                o.type_name()
                            ));
                        }
                    };
                    m.insert(n, j);
                }
                Json::Object(m)
            }
            "providers" => to_json(&v, &pfx, true, None)?,
            _ => to_json(&v, &pfx, false, None)?,
        };
        obj.insert(k, j);
    }
    let val = Json::Object(obj);
    from_value(
        val.clone(),
        &ns.map_or_else(|| src.into(), |n| format!("{src} ({n})")),
    )?;
    Ok(val)
}

fn req_fn(t: &Table, pfx: &str) -> Result<Function, String> {
    match t
        .raw_get::<Value>("run")
        .map_err(|e| format!("{pfx}.run: {e}"))?
    {
        Value::Function(f) => Ok(f),
        Value::Nil => Err(format!("{pfx}.run: is required")),
        o => Err(format!(
            "{pfx}.run: expected function, got {}",
            o.type_name()
        )),
    }
}

fn val_entry(p: &Table, allowed: &[&str], pfx: &str) -> Result<(Vec<String>, Function), String> {
    let mut keys: Vec<_> = p
        .pairs::<Value, Value>()
        .flatten()
        .map(|(k, _)| key_str(&k))
        .collect();
    keys.sort();
    if let Some(k) = keys.into_iter().find(|k| !allowed.contains(&k.as_str())) {
        return Err(format!("{pfx}.{k}: unknown key"));
    }
    let run = req_fn(p, pfx)?;
    let args = match p
        .raw_get::<Value>("args")
        .map_err(|e| format!("{pfx}.args: {e}"))?
    {
        Value::Nil => vec![],
        v => parse_str_list(&v, &format!("{pfx}.args"), "arg", true)?,
    };
    Ok((args, run))
}

fn to_json(v: &Value, path: &str, mask: bool, skip: Option<&str>) -> Result<Json, String> {
    let err = |m: &str| {
        Err(format!(
            "{path}: {}",
            if mask { "invalid provider settings" } else { m }
        ))
    };
    match v {
        Value::Nil => Ok(Json::Null),
        Value::Boolean(b) => Ok(Json::Bool(*b)),
        Value::Integer(n) => Ok(Json::Number((*n).into())),
        Value::Number(n) => serde_json::Number::from_f64(*n)
            .map(Json::Number)
            .ok_or_else(|| err("expected a finite number").unwrap_err()),
        Value::String(s) => Ok(Json::String(s.to_string_lossy())),
        Value::Table(t) => {
            let (mut ints, mut strs) = (BTreeMap::new(), BTreeMap::new());
            for pair in t.pairs::<Value, Value>() {
                let (k, val) = pair.map_err(|e| format!("{path}: {e}"))?;
                if let Some(i) = lua_index(&k) {
                    ints.insert(i, val);
                } else if let Value::String(s) = k {
                    let s = s.to_string_lossy();
                    if skip != Some(s.as_str()) {
                        strs.insert(s, val);
                    }
                } else {
                    return err(&format!("unsupported table key `{}`", key_str(&k)));
                }
            }
            if !ints.is_empty() && !strs.is_empty() {
                return err("expected a list or string-keyed table");
            }
            if !ints.is_empty() {
                let mut arr = Vec::with_capacity(ints.len());
                for (want, (i, val)) in (1..).zip(ints) {
                    if i != want {
                        return err(&format!("list has hole at index {want}"));
                    }
                    arr.push(to_json(&val, &format!("{path}[{i}]"), mask, None)?);
                }
                return Ok(Json::Array(arr));
            }
            let mut map = Map::new();
            for (k, val) in strs {
                map.insert(
                    k.clone(),
                    to_json(&val, &format!("{path}.{k}"), mask, None)?,
                );
            }
            Ok(Json::Object(map))
        }
        o => err(&format!("unsupported value type {}", o.type_name())),
    }
}

fn claim(
    seen: &mut BTreeMap<(&str, String), String>,
    sec: &'static str,
    k: &str,
    src: &str,
) -> Result<(), String> {
    match seen.insert((sec, k.into()), src.into()) {
        Some(prev) => Err(format!("{sec}.{k}: duplicate key in `{prev}` and `{src}`")),
        None => Ok(()),
    }
}

fn merge_raw(lua: &Lua, items: &[Contribution]) -> Result<(Map<String, Json>, Table), String> {
    let e = |x: tuse_lua::mlua::Error| x.to_string();
    let (mut merged, merged_lua, mut seen) =
        (Map::new(), lua.create_table().map_err(e)?, BTreeMap::new());
    let (mut models, mut systems, mut hide, mut open) = (vec![], vec![], vec![], vec![]);
    let lua_secs = ["functions", "commands", "prompts", "hooks", "open"];
    for sec in lua_secs {
        merged_lua
            .raw_set(sec, lua.create_table().map_err(e)?)
            .map_err(e)?;
    }
    for item in items {
        let Some(obj) = item.json.as_object() else {
            continue;
        };
        for sec in ["functions", "commands", "prompts", "providers", "hooks"] {
            if let Some(Json::Object(m)) = obj.get(sec) {
                let dst = merged
                    .entry(sec)
                    .or_insert_with(|| json!({}))
                    .as_object_mut()
                    .unwrap();
                for (k, v) in m {
                    claim(&mut seen, sec, k, &item.source)?;
                    dst.insert(k.clone(), v.clone());
                }
            }
        }
        for sec in lua_secs {
            if let Ok(Value::Table(s)) = item.table.raw_get::<Value>(sec) {
                let d: Table = merged_lua.raw_get(sec).map_err(e)?;
                for pair in s.pairs::<Value, Value>() {
                    let (k, v) = pair.map_err(e)?;
                    d.raw_set(k, v).map_err(e)?;
                }
            }
        }
        for k in obj
            .get("open")
            .and_then(Json::as_array)
            .into_iter()
            .flatten()
            .filter_map(Json::as_str)
        {
            claim(&mut seen, "open", k, &item.source)?;
            open.push(Json::String(k.into()));
        }
        for m in obj
            .get("models")
            .and_then(Json::as_array)
            .into_iter()
            .flatten()
        {
            if let Some(n) = m.get("name").and_then(Json::as_str) {
                claim(&mut seen, "models", n, &item.source)?;
            }
            models.push(m.clone());
        }
        systems.extend(
            obj.get("systems")
                .and_then(Json::as_array)
                .into_iter()
                .flatten()
                .cloned(),
        );
        hide.extend(
            obj.get("history")
                .and_then(|h| h.get("hide"))
                .and_then(Json::as_array)
                .into_iter()
                .flatten()
                .cloned(),
        );
    }
    let mut ui = Map::new();
    for layer in 0u8..=2u8 {
        let mut leaves = BTreeMap::new();
        for item in items.iter().filter(|i| i.layer == layer) {
            if let Some(Json::Object(u)) = item.json.as_object().and_then(|o| o.get("ui")) {
                merge_ui(&mut ui, u, "", &item.source, &mut leaves)?;
            }
        }
    }
    open.sort_by(|a, b| a.as_str().cmp(&b.as_str()));
    if !ui.is_empty() {
        merged.insert("ui".into(), Json::Object(ui));
    }
    if !open.is_empty() {
        merged.insert("open".into(), Json::Array(open));
    }
    if !models.is_empty() {
        merged.insert("models".into(), Json::Array(models));
    }
    if !systems.is_empty() {
        merged.insert("systems".into(), Json::Array(systems));
    }
    if !hide.is_empty() {
        merged.insert("history".into(), json!({ "hide": hide }));
    }
    Ok((merged, merged_lua))
}

pub(crate) fn merge_contributions(
    lua: &Lua,
    items: &[Contribution],
    src: &str,
) -> Result<(Config, Table), String> {
    let (merged, merged_lua) = merge_raw(lua, items)?;
    Ok((from_value(Json::Object(merged), src)?, merged_lua))
}
pub(crate) fn merge_plugins_lua(lua: &Lua, items: &[Contribution]) -> Result<Table, String> {
    let (merged, merged_lua) = merge_raw(lua, items)?;
    from_value(Json::Object(merged.clone()), "plugins")?;
    for (k, def) in [
        ("ui", json!({})),
        ("providers", json!({})),
        ("models", json!([])),
        ("systems", json!([])),
        ("history", json!({ "hide": [] })),
    ] {
        let lv = tuse_lua::to_lua(lua, merged.get(k).unwrap_or(&def)).map_err(|e| e.to_string())?;
        merged_lua.raw_set(k, lv).map_err(|e| e.to_string())?;
    }
    Ok(merged_lua)
}
fn merge_ui(
    dst: &mut Map<String, Json>,
    src: &Map<String, Json>,
    pfx: &str,
    source: &str,
    leaves: &mut BTreeMap<String, String>,
) -> Result<(), String> {
    for k in src.keys().collect::<BTreeSet<_>>() {
        let leaf = if pfx.is_empty() {
            k.clone()
        } else {
            format!("{pfx}.{k}")
        };
        if let Json::Object(sub) = &src[k] {
            let e = dst.entry(k.clone()).or_insert_with(|| json!({}));
            if !e.is_object() {
                *e = json!({});
            }
            merge_ui(e.as_object_mut().unwrap(), sub, &leaf, source, leaves)?;
        } else if let Some(prev) = leaves.insert(leaf.clone(), source.into()) {
            return Err(format!(
                "ui.{leaf}: duplicate key in `{prev}` and `{source}`"
            ));
        } else {
            dst.insert(k.clone(), src[k].clone());
        }
    }
    Ok(())
}
fn str_entries(t: &Table, src: &str, sec: Option<&str>) -> Result<Vec<(String, Value)>, String> {
    let mut out = vec![];
    for pair in t.pairs::<Value, Value>() {
        let (k, v) = pair.map_err(|e| format!("{src}: {e}"))?;
        let Value::String(s) = k else {
            let ks = key_str(&k);
            let p = sec.map_or(ks.clone(), |s| format!("{s}.{ks}"));
            return Err(format!("{src}: {p}: expected string key"));
        };
        out.push((s.to_string_lossy(), v));
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(out)
}
fn key_str(v: &Value) -> String {
    match v {
        Value::String(s) => s.to_string_lossy(),
        Value::Integer(n) => n.to_string(),
        Value::Number(n) => n.to_string(),
        o => format!("<{}>", o.type_name()),
    }
}
