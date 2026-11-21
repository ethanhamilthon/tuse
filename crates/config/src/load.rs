use crate::{Mode, expand, mode_matches, types::Config};
use serde_yaml::{Mapping, Value};
use std::path::{Path, PathBuf};

pub struct Loaded {
    pub config: Config,
    /// Every file read, includes first.
    pub files: Vec<PathBuf>,
    pub path: Option<PathBuf>,
}

pub fn home() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default()
}

/// The global config: `$TUSE_GLOBAL_CONFIG`, else `~/.config/tuse/main.yaml`.
pub fn global_path() -> PathBuf {
    match std::env::var_os("TUSE_GLOBAL_CONFIG").filter(|v| !v.is_empty()) {
        Some(p) => PathBuf::from(p),
        None => home().join(".config/tuse/main.yaml"),
    }
}

/// `--config PATH`, else `./tuse.yaml`, else the global config.
pub fn find(explicit: Option<&Path>, cwd: &Path) -> Option<PathBuf> {
    find_in(explicit, cwd, &global_path())
}

pub(crate) fn find_in(explicit: Option<&Path>, cwd: &Path, global: &Path) -> Option<PathBuf> {
    if let Some(p) = explicit {
        return Some(p.to_path_buf());
    }
    [cwd.join("tuse.yaml"), global.to_path_buf()]
        .into_iter()
        .find(|p| p.is_file())
}

/// Loads `path` and its includes for `mode`; `None` gives the built-in defaults.
pub fn load(path: Option<&Path>, mode: Mode) -> Result<Loaded, String> {
    let mut files = vec![];
    let mut value = Value::Mapping(Mapping::new());
    if let Some(path) = path {
        merge(&mut value, load_file(path, &mut files, 0, mode)?);
    }
    let name = path.map_or("config".to_string(), |p| p.display().to_string());
    if value.get("prelude").is_some() {
        return Err(format!(
            "{name}: `prelude` is no longer supported; use hooks or `import`"
        ));
    }
    expand::walk(&mut value);
    crate::themes::resolve(&mut value).map_err(|e| format!("{name}: {e}"))?;
    let config: Config = serde_yaml::from_value(value).map_err(|e| format!("{name}: {e}"))?;
    config
        .settings
        .ui
        .check()
        .map_err(|e| format!("{name}: settings.ui: {e}"))?;
    crate::functions::validate(&config).map_err(|e| format!("{name}: {e}"))?;
    let theme = config.theme_name();
    if !matches!(theme, "" | "default") && !config.themes.contains_key(theme) {
        return Err(format!("{name}: theme {theme} not found"));
    }
    Ok(Loaded {
        config,
        files,
        path: path.map(Path::to_path_buf),
    })
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct IncludeSpec {
    file: String,
    #[serde(default)]
    only: Option<Vec<String>>,
    #[serde(default)]
    modes: Vec<Mode>,
}

fn parse_include(v: Value, path: &Path) -> Result<IncludeSpec, String> {
    let spec = match v {
        Value::String(file) => IncludeSpec {
            file,
            only: None,
            modes: vec![],
        },
        Value::Mapping(_) => serde_yaml::from_value::<IncludeSpec>(v)
            .map_err(|e| format!("{}: include: {e}", path.display()))?,
        _ => {
            return Err(format!(
                "{}: include entries must be a path or {{file, only, modes}}",
                path.display()
            ));
        }
    };
    if spec.file.trim().is_empty() {
        return Err(format!(
            "{}: include: `file` must not be empty",
            path.display()
        ));
    }
    Ok(spec)
}

fn load_file(
    path: &Path,
    files: &mut Vec<PathBuf>,
    depth: usize,
    mode: Mode,
) -> Result<Value, String> {
    if depth > 16 {
        return Err(format!("{}: include depth limit reached", path.display()));
    }
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut own: Value = if text.trim().is_empty() {
        Value::Mapping(Mapping::new())
    } else {
        serde_yaml::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?
    };
    if !own.is_mapping() {
        return Err(format!("{}: top level must be a mapping", path.display()));
    }
    let dir = path.parent().unwrap_or(Path::new(".")).to_path_buf();
    let includes = match own.as_mapping_mut().and_then(|m| m.remove("include")) {
        None | Some(Value::Null) => vec![],
        Some(Value::Sequence(seq)) => seq,
        Some(_) => return Err(format!("{}: `include` must be a list", path.display())),
    };
    let mut merged = Value::Mapping(Mapping::new());
    for inc in includes {
        let spec = parse_include(inc, path)?;
        if !mode_matches(&spec.modes, mode) {
            continue;
        }
        let inc_path = resolve_path(&spec.file, &dir);
        let mut v = load_file(&inc_path, files, depth + 1, mode)?;
        if let Some(only) = &spec.only
            && let Value::Mapping(m) = &mut v
        {
            m.retain(|k, _| k.as_str().is_some_and(|s| only.iter().any(|o| o == s)));
        }
        merge(&mut merged, v);
    }
    merge(&mut merged, own);
    files.push(path.to_path_buf());
    Ok(merged)
}

/// Mappings merge by key, lists are concatenated, anything else: `b` wins.
pub fn merge(a: &mut Value, b: Value) {
    match (a, b) {
        (Value::Mapping(a), Value::Mapping(b)) => {
            for (k, v) in b {
                match a.get_mut(&k) {
                    Some(old) => merge(old, v),
                    None => {
                        a.insert(k, v);
                    }
                }
            }
        }
        (Value::Sequence(a), Value::Sequence(b)) => a.extend(b),
        (a, b) => *a = b,
    }
}

pub fn resolve_path(p: &str, dir: &Path) -> PathBuf {
    if let Some(rest) = p.strip_prefix("~/") {
        return home().join(rest);
    }
    let p = Path::new(p);
    if p.is_absolute() {
        p.to_path_buf()
    } else {
        dir.join(p)
    }
}
