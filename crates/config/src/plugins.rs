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
    #[serde(default)]
    pub lazy: bool,
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

static PLUGIN_LOCK: std::sync::RwLock<()> = std::sync::RwLock::new(());

pub struct PluginLock {
    _file: Option<std::fs::File>,
    _guard: LockGuard,
}

#[allow(dead_code)]
enum LockGuard {
    Read(std::sync::RwLockReadGuard<'static, ()>),
    Write(std::sync::RwLockWriteGuard<'static, ()>),
}

fn lock_file_path(dir: &Path) -> PathBuf {
    dir.with_file_name(".plugins.lock")
}

fn open_lock(lp: &Path) -> std::io::Result<std::fs::File> {
    std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(lp)
}

fn acquire_flock(file: &std::fs::File, exclusive: bool) -> std::io::Result<()> {
    loop {
        let res = if exclusive {
            file.lock()
        } else {
            file.lock_shared()
        };
        match res {
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            other => return other,
        }
    }
}

pub fn plugins_read_lock(dir: &Path) -> Result<Option<PluginLock>, String> {
    let guard = PLUGIN_LOCK.read().unwrap_or_else(|e| e.into_inner());
    let Some(parent) = dir.parent() else {
        return Ok(Some(PluginLock {
            _file: None,
            _guard: LockGuard::Read(guard),
        }));
    };
    if !parent.is_dir() {
        return Ok(None);
    }
    let lp = lock_file_path(dir);
    let file = match open_lock(&lp) {
        Ok(f) => Some(f),
        Err(e) => match std::fs::File::open(&lp) {
            Ok(f) => Some(f),
            Err(e2)
                if e2.kind() == std::io::ErrorKind::NotFound
                    && matches!(
                        e.kind(),
                        std::io::ErrorKind::PermissionDenied
                            | std::io::ErrorKind::ReadOnlyFilesystem
                    ) =>
            {
                None
            }
            Err(_) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(_) => return Err(format!("{}: {e}", lp.display())),
        },
    };
    if let Some(f) = &file {
        acquire_flock(f, false).map_err(|e| format!("{}: {e}", lp.display()))?;
    }
    Ok(Some(PluginLock {
        _file: file,
        _guard: LockGuard::Read(guard),
    }))
}

pub fn plugins_write_lock(dir: &Path) -> Result<PluginLock, String> {
    let guard = PLUGIN_LOCK.write().unwrap_or_else(|e| e.into_inner());
    if let Some(parent) = dir.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
    }
    let lp = lock_file_path(dir);
    let file = open_lock(&lp).map_err(|e| format!("{}: {e}", lp.display()))?;
    acquire_flock(&file, true).map_err(|e| format!("{}: {e}", lp.display()))?;
    Ok(PluginLock {
        _file: Some(file),
        _guard: LockGuard::Write(guard),
    })
}

pub fn discover_plugins_unlocked(dir: &Path) -> Result<Vec<InstalledPlugin>, String> {
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
            && let Some(name) = path.file_name().and_then(|n| n.to_str())
            && ![".staging-", ".backup-", ".trash-"]
                .iter()
                .any(|p| name.starts_with(p))
            && path.join("tuse-plugin.json").is_file()
        {
            out.push(load_plugin_dir(name, &path)?);
        }
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

pub fn discover_plugins_in(dir: &Path) -> Result<Vec<InstalledPlugin>, String> {
    let Some(_lock) = plugins_read_lock(dir)? else {
        return Ok(vec![]);
    };
    discover_plugins_unlocked(dir)
}

pub fn discover_plugins() -> Result<Vec<InstalledPlugin>, String> {
    discover_plugins_in(&crate::plugins_dir())
}
