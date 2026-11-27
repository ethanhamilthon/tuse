use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::{Component, Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginSummary {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginManifest {
    pub version: String,
    #[serde(default = "default_entrypoint")]
    pub entrypoint: String,
    #[serde(default)]
    pub name: Option<String>,
}

fn default_entrypoint() -> String {
    "init.lua".into()
}

#[derive(Debug, Clone)]
pub struct InstalledPlugin {
    pub name: String,
    pub dir: PathBuf,
    pub entrypoint: PathBuf,
    pub manifest: PluginManifest,
    pub raw_manifest: Value,
}

pub fn validate_plugin_name(name: &str) -> Result<(), String> {
    let bad = name.is_empty()
        || matches!(name, "." | "..")
        || name
            .chars()
            .any(|c| matches!(c, '/' | '\\' | ':') || c.is_whitespace() || c.is_control());
    if bad {
        return Err(format!("invalid plugin name `{name}`"));
    }
    Ok(())
}

pub fn resolve_entrypoint(root: &Path, entry: &str) -> Result<PathBuf, String> {
    if entry.trim().is_empty() {
        return Err("entrypoint must not be empty".into());
    }
    let rel = Path::new(entry);
    if rel.is_absolute()
        || rel
            .components()
            .any(|c| !matches!(c, Component::Normal(_) | Component::CurDir))
    {
        return Err(format!("entrypoint `{entry}` escapes plugin root"));
    }
    let target = root.join(rel);
    if !target.is_file() {
        return Err(format!("entrypoint `{entry}` not found"));
    }
    let canon_root = root
        .canonicalize()
        .map_err(|e| format!("{}: {e}", root.display()))?;
    let canon_target = target
        .canonicalize()
        .map_err(|e| format!("{}: {e}", target.display()))?;
    if !canon_target.starts_with(&canon_root) {
        return Err(format!("entrypoint `{entry}` escapes plugin root"));
    }
    Ok(canon_target)
}

pub fn load_plugin_dir(name: &str, dir: &Path) -> Result<InstalledPlugin, String> {
    validate_plugin_name(name)?;
    let pfx = |m: String| format!("plugin `{name}`: tuse-plugin.json: {m}");
    let mp = dir.join("tuse-plugin.json");
    let text = std::fs::read_to_string(&mp)
        .map_err(|e| format!("plugin `{name}`: {}: {e}", mp.display()))?;
    let raw_manifest: Value = serde_json::from_str(&text)
        .map_err(|e| format!("plugin `{name}`: invalid tuse-plugin.json: {e}"))?;
    let manifest: PluginManifest = serde_json::from_value(raw_manifest.clone())
        .map_err(|e| format!("plugin `{name}`: invalid tuse-plugin.json: {e}"))?;
    if manifest.version.trim().is_empty() {
        return Err(pfx("version must not be empty".into()));
    }
    if let Some(mname) = manifest.name.as_deref() {
        validate_plugin_name(mname).map_err(pfx)?;
    }
    let entrypoint = resolve_entrypoint(dir, &manifest.entrypoint).map_err(pfx)?;
    Ok(InstalledPlugin {
        name: name.into(),
        dir: dir.into(),
        entrypoint,
        manifest,
        raw_manifest,
    })
}

pub fn discover_plugins_in(dir: &Path) -> Result<Vec<InstalledPlugin>, String> {
    if !dir.is_dir() {
        return Ok(vec![]);
    }
    let mut out = vec![];
    for entry in std::fs::read_dir(dir)
        .map_err(|e| format!("{}: {e}", dir.display()))?
        .flatten()
    {
        let path = entry.path();
        if path.is_dir()
            && path.join("tuse-plugin.json").is_file()
            && let Some(name) = path.file_name().and_then(|n| n.to_str())
        {
            out.push(load_plugin_dir(name, &path)?);
        }
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

pub fn discover_plugins() -> Result<Vec<InstalledPlugin>, String> {
    discover_plugins_in(&crate::plugins_dir())
}
