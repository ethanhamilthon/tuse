use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use tuse_config::{
    InstalledPlugin, discover_plugins_unlocked, plugins_dir, plugins_read_lock, plugins_write_lock,
};

static WORK_SEQ: AtomicU64 = AtomicU64::new(0);

fn work_dir(root: &Path) -> PathBuf {
    root.with_file_name(".plugins-work")
}

fn work_entry(root: &Path, prefix: &str) -> PathBuf {
    let seq = WORK_SEQ.fetch_add(1, Ordering::Relaxed);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    work_dir(root).join(format!("{prefix}-{}-{nanos}-{seq}", std::process::id()))
}

fn meta_path(root: &Path) -> PathBuf {
    root.with_file_name("plugins.json")
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PluginMeta {
    source: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PluginInfo {
    pub name: String,
    pub version: String,
    pub entrypoint: String,
    pub path: String,
    pub source: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PluginDetail {
    pub name: String,
    pub version: String,
    pub entrypoint: String,
    pub path: String,
    pub source: Option<String>,
    pub manifest: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PluginAddResult {
    pub name: String,
    pub version: String,
    pub entrypoint: String,
    pub path: String,
    pub source: String,
    pub replaced: bool,
}

struct Repo {
    source: String,
    owner: String,
    repo: String,
    subdir: Option<String>,
    git_ref: Option<String>,
}

pub struct Plugins;

impl Plugins {
    pub fn list() -> Result<Vec<PluginInfo>, String> {
        let root = plugins_dir();
        let Some(_lock) = plugins_read_lock(&root)? else {
            return Ok(vec![]);
        };
        let plugins = discover_plugins_unlocked(&root)?;
        let meta = read_meta(&root);
        Ok(plugins.iter().map(|p| to_info(p, &meta)).collect())
    }

    pub fn get(target: &str) -> Result<PluginDetail, String> {
        let root = plugins_dir();
        let Some(_lock) = plugins_read_lock(&root)? else {
            return Err(format!("plugin `{target}` is not installed"));
        };
        let plugins = discover_plugins_unlocked(&root)?;
        let meta = read_meta(&root);
        let p = find(&plugins, &meta, target)?;
        Ok(PluginDetail {
            name: p.name.clone(),
            version: p.manifest.version.clone(),
            entrypoint: p.manifest.entrypoint.clone(),
            path: p
                .dir
                .canonicalize()
                .map_err(|e| pe(&p.dir, e))?
                .display()
                .to_string(),
            source: meta.get(&p.name).map(|m| m.source.clone()),
            manifest: p.raw_manifest.clone(),
        })
    }

    pub(crate) fn entrypoint(name: &str) -> Result<PathBuf, String> {
        let root = plugins_dir();
        let missing = || format!("plugin `{name}` is not installed");
        let Some(_lock) = plugins_read_lock(&root)? else {
            return Err(missing());
        };
        discover_plugins_unlocked(&root)?
            .into_iter()
            .find(|p| p.name == name)
            .map(|p| p.entrypoint)
            .ok_or_else(missing)
    }

    pub fn add(source: &str, replace: bool) -> Result<PluginAddResult, String> {
        let r = parse_repo(source)?;
        let url = archive_url(&r);
        let mut bytes = vec![];
        ureq::get(&url)
            .set("User-Agent", concat!("tuse/", env!("CARGO_PKG_VERSION")))
            .call()
            .map_err(|e| format!("{url}: {e}"))?
            .into_reader()
            .read_to_end(&mut bytes)
            .map_err(|e| format!("{url}: {e}"))?;
        let root = plugins_dir();
        let work = work_dir(&root);
        std::fs::create_dir_all(&work).map_err(|e| pe(&work, e))?;
        let staging = loop {
            let dir = work_entry(&root, "staging");
            match std::fs::create_dir(&dir) {
                Ok(()) => break dir,
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(pe(&dir, e)),
            }
        };
        let mut cleanup_bak: Option<PathBuf> = None;
        let res = (|| -> Result<PluginAddResult, String> {
            extract_tarball(&bytes, r.subdir.as_deref(), &staging)?;
            let fb = r
                .subdir
                .as_deref()
                .and_then(|s| s.rsplit('/').next())
                .unwrap_or(&r.repo);
            let parsed = tuse_config::load_plugin_dir(fb, &staging)?;
            let name = parsed.manifest.name.clone().unwrap_or_else(|| fb.into());
            tuse_config::validate_plugin_name(&name)?;
            let dest = root.join(&name);

            let _lock = plugins_write_lock(&root)?;
            std::fs::create_dir_all(&root).map_err(|e| pe(&root, e))?;
            let existed = dest.exists();
            if existed && !replace {
                return Err(format!(
                    "plugin `{name}` already exists; pass `replace = true` or delete `{name}` first"
                ));
            }
            let backup = existed.then(|| work_entry(&root, "backup"));
            if let Some(bak) = &backup {
                std::fs::rename(&dest, bak).map_err(|e| pe(&dest, e))?;
            }
            if let Err(e) = std::fs::rename(&staging, &dest) {
                if let Some(bak) = &backup {
                    let _ = std::fs::rename(bak, &dest);
                }
                return Err(pe(&dest, e));
            }
            let mut meta = read_meta(&root);
            meta.insert(
                name.clone(),
                PluginMeta {
                    source: r.source.clone(),
                },
            );
            if let Err(e) = write_meta(&root, &meta) {
                let _ = std::fs::rename(&dest, &staging);
                if let Some(bak) = &backup {
                    let _ = std::fs::rename(bak, &dest);
                }
                return Err(e);
            }
            cleanup_bak = backup;
            Ok(PluginAddResult {
                name,
                version: parsed.manifest.version,
                entrypoint: parsed.manifest.entrypoint,
                path: dest.display().to_string(),
                source: r.source,
                replaced: existed,
            })
        })();
        if res.is_err() {
            let _ = std::fs::remove_dir_all(&staging);
        }
        if let Some(bak) = cleanup_bak {
            let _ = std::fs::remove_dir_all(bak);
        }
        res
    }

    pub fn delete(target: &str) -> Result<String, String> {
        let root = plugins_dir();
        let (name, trash) = {
            let _lock = plugins_write_lock(&root)?;
            let plugins = discover_plugins_unlocked(&root)?;
            let mut meta = read_meta(&root);
            let name = find(&plugins, &meta, target)?.name.clone();
            let dest = root.join(&name);
            let trash = work_entry(&root, "trash");
            if let Some(parent) = trash.parent() {
                std::fs::create_dir_all(parent).map_err(|e| pe(parent, e))?;
            }
            std::fs::rename(&dest, &trash).map_err(|e| pe(&dest, e))?;
            meta.remove(&name);
            if let Err(e) = write_meta(&root, &meta) {
                let _ = std::fs::rename(&trash, &dest);
                return Err(e);
            }
            (name, trash)
        };
        let _ = std::fs::remove_dir_all(trash);
        Ok(name)
    }

    pub fn update(target: Option<&str>) -> Result<Vec<PluginInfo>, String> {
        let root = plugins_dir();
        let (targets, meta) = {
            let Some(_lock) = plugins_read_lock(&root)? else {
                return match target {
                    Some(t) => Err(format!("plugin `{t}` is not installed")),
                    None => Ok(vec![]),
                };
            };
            let plugins = discover_plugins_unlocked(&root)?;
            let meta = read_meta(&root);
            let list: Vec<String> = match target {
                Some(t) => vec![find(&plugins, &meta, t)?.name.clone()],
                None if plugins.is_empty() => return Ok(vec![]),
                None => plugins.into_iter().map(|p| p.name).collect(),
            };
            (list, meta)
        };
        let mut updated = vec![];
        let mut errs = vec![];
        for name in targets {
            match meta.get(&name) {
                Some(m) => match Self::add(&m.source, true) {
                    Ok(res) => updated.push(PluginInfo {
                        name: res.name,
                        version: res.version,
                        entrypoint: res.entrypoint,
                        path: res.path,
                        source: Some(res.source),
                    }),
                    Err(e) => errs.push(format!("{name}: {e}")),
                },
                None => errs.push(format!("plugin `{name}` has no recorded source")),
            }
        }
        if !errs.is_empty() {
            return Err(errs.join("\n"));
        }
        Ok(updated)
    }
}

fn to_info(p: &InstalledPlugin, meta: &BTreeMap<String, PluginMeta>) -> PluginInfo {
    PluginInfo {
        name: p.name.clone(),
        version: p.manifest.version.clone(),
        entrypoint: p.manifest.entrypoint.clone(),
        path: p.dir.display().to_string(),
        source: meta.get(&p.name).map(|m| m.source.clone()),
    }
}

fn find<'a>(
    plugins: &'a [InstalledPlugin],
    meta: &BTreeMap<String, PluginMeta>,
    target: &str,
) -> Result<&'a InstalledPlugin, String> {
    plugins
        .iter()
        .find(|p| p.name == target || meta.get(&p.name).is_some_and(|m| m.source == target))
        .ok_or_else(|| format!("plugin `{target}` is not installed"))
}

fn pe(p: &Path, e: impl std::fmt::Display) -> String {
    format!("{}: {e}", p.display())
}

fn archive_url(r: &Repo) -> String {
    match (
        std::env::var("TUSE_PLUGINS_URL")
            .ok()
            .filter(|b| !b.trim().is_empty()),
        r.git_ref.as_deref(),
    ) {
        (Some(b), Some(g)) => format!(
            "{}/{}/{}/{g}.tar.gz",
            b.trim().trim_end_matches('/'),
            r.owner,
            r.repo
        ),
        (Some(b), None) => format!(
            "{}/{}/{}.tar.gz",
            b.trim().trim_end_matches('/'),
            r.owner,
            r.repo
        ),
        (None, Some(g)) => format!(
            "https://codeload.github.com/{}/{}/tar.gz/{g}",
            r.owner, r.repo
        ),
        (None, None) => format!(
            "https://api.github.com/repos/{}/{}/tarball",
            r.owner, r.repo
        ),
    }
}

fn parse_repo(raw: &str) -> Result<Repo, String> {
    let trimmed = raw.trim().trim_end_matches('/');
    let gh = trimmed
        .strip_prefix("https://github.com/")
        .or_else(|| trimmed.strip_prefix("http://github.com/"));
    let (path_part, mut git_ref) = match gh {
        Some(r) => (r, None),
        None => match trimmed.split_once('@') {
            Some((p, r)) if !r.is_empty() => (p, Some(r.into())),
            Some(_) => return Err(format!("empty ref in `{raw}`")),
            None => (trimmed, None),
        },
    };
    let p: Vec<_> = path_part.split('/').filter(|s| !s.is_empty()).collect();
    if p.len() < 2 {
        return Err(format!(
            "expected owner/repo[/subdir][@ref] or GitHub URL, got `{raw}`"
        ));
    }
    let (owner, repo) = (
        p[0].to_string(),
        p[1].strip_suffix(".git").unwrap_or(p[1]).to_string(),
    );
    let subdir = match (gh.is_some(), p.get(2).copied()) {
        (true, None) => None,
        (true, Some("tree")) if p.len() >= 4 => {
            git_ref = Some(p[3].into());
            (p.len() > 4).then(|| p[4..].join("/"))
        }
        (true, _) => return Err(format!("unsupported GitHub URL `{raw}`")),
        (false, _) => (p.len() > 2).then(|| p[2..].join("/")),
    };
    if owner == ".."
        || repo == ".."
        || subdir
            .as_deref()
            .is_some_and(|s| s.split('/').any(|x| matches!(x, "" | "." | "..")))
    {
        return Err(format!("invalid repository `{raw}`"));
    }
    Ok(Repo {
        source: trimmed.into(),
        owner,
        repo,
        subdir,
        git_ref,
    })
}

fn extract_tarball(bytes: &[u8], subdir: Option<&str>, dst: &Path) -> Result<(), String> {
    let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(bytes));
    let sub: Vec<_> = subdir
        .map(|s| s.split('/').filter(|p| !p.is_empty()).collect())
        .unwrap_or_default();
    let mut n = 0usize;
    for entry in archive.entries().map_err(|e| format!("archive: {e}"))? {
        let mut entry = entry.map_err(|e| format!("archive: {e}"))?;
        let kind = entry.header().entry_type();
        if kind.is_symlink() || kind.is_hard_link() {
            return Err("archive contains disallowed link".into());
        }
        let path = entry.path().map_err(|e| format!("archive: {e}"))?;
        let comps: Vec<_> = path
            .components()
            .filter(|c| !matches!(c, Component::CurDir))
            .map(|c| match c {
                Component::Normal(s) => Ok(s.to_os_string()),
                _ => Err(format!("archive entry escapes root: {}", path.display())),
            })
            .collect::<Result<_, _>>()?;
        let Some(inner) = comps.get(1..) else {
            continue;
        };
        let rel = if sub.is_empty() {
            inner
        } else if inner.len() >= sub.len()
            && sub.iter().zip(inner).all(|(a, b)| b.to_str() == Some(*a))
        {
            &inner[sub.len()..]
        } else {
            continue;
        };
        if rel.is_empty() {
            continue;
        }
        let target = rel.iter().fold(dst.to_path_buf(), |p, c| p.join(c));
        if kind.is_dir() {
            std::fs::create_dir_all(&target).map_err(|e| pe(&target, e))?;
        } else if kind.is_file() {
            if let Some(p) = target.parent() {
                std::fs::create_dir_all(p).map_err(|e| pe(p, e))?;
            }
            let mut out = std::fs::File::create(&target).map_err(|e| pe(&target, e))?;
            std::io::copy(&mut entry, &mut out).map_err(|e| pe(&target, e))?;
            n += 1;
        }
    }
    if n == 0 {
        return Err(subdir.map_or_else(
            || "archive is empty".into(),
            |s| format!("subdirectory `{s}` not found"),
        ));
    }
    Ok(())
}

fn read_meta(root: &Path) -> BTreeMap<String, PluginMeta> {
    std::fs::read_to_string(meta_path(root))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn write_meta(root: &Path, map: &BTreeMap<String, PluginMeta>) -> Result<(), String> {
    let path = meta_path(root);
    if let Some(p) = path.parent() {
        std::fs::create_dir_all(p).map_err(|e| pe(p, e))?;
    }
    let body = serde_json::to_string_pretty(map).map_err(|e| e.to_string())?;
    let seq = WORK_SEQ.fetch_add(1, Ordering::Relaxed);
    let tmp = path.with_file_name(format!(".plugins.json.tmp.{}-{seq}", std::process::id()));
    std::fs::write(&tmp, body).map_err(|e| pe(&tmp, e))?;
    std::fs::rename(&tmp, &path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        pe(&path, e)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::Compression;
    use flate2::write::GzEncoder;
    use std::io::{BufRead, BufReader, Write};
    use std::net::TcpListener;
    use std::sync::Arc;

    fn tgz(files: &[(&str, &str)]) -> Vec<u8> {
        let mut enc = GzEncoder::new(Vec::new(), Compression::default());
        {
            let mut tar = tar::Builder::new(&mut enc);
            for (path, body) in files {
                let mut h = tar::Header::new_gnu();
                h.set_size(body.len() as u64);
                h.set_mode(0o644);
                h.set_cksum();
                tar.append_data(&mut h, path, body.as_bytes()).unwrap();
            }
            tar.finish().unwrap();
        }
        enc.finish().unwrap()
    }

    fn serve(routes: Vec<(String, Vec<u8>)>) -> String {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", l.local_addr().unwrap());
        let routes = Arc::new(routes);
        std::thread::spawn(move || {
            for mut s in l.incoming().flatten() {
                let mut r = BufReader::new(s.try_clone().unwrap());
                let mut req = String::new();
                let _ = r.read_line(&mut req);
                let path = req.split_whitespace().nth(1).unwrap_or("");
                let mut line = String::new();
                while r.read_line(&mut line).unwrap_or(0) > 0 && !line.trim().is_empty() {
                    line.clear();
                }
                if let Some((_, body)) = routes.iter().find(|(p, _)| p == path) {
                    let hdr = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/gzip\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        body.len()
                    );
                    let _ = s.write_all(hdr.as_bytes());
                    let _ = s.write_all(body);
                } else {
                    let _ = s.write_all(
                        b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                    );
                }
            }
        });
        url
    }

    #[test]
    fn plugin_service_lifecycle_refs_manifest_and_safe_replace() {
        let _g = crate::storage::ENV_LOCK.lock().unwrap();
        let home = std::env::temp_dir().join(format!("tuse-plug-svc-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&home);
        let good = tgz(&[
            (
                "r-1/tuse-plugin.json",
                r#"{"name":"demo","version":"1.0.0"}"#,
            ),
            ("r-1/init.lua", "return {}"),
            ("r-1/packs/sub/tuse-plugin.json", r#"{"version":"0.2.0"}"#),
            ("r-1/packs/sub/init.lua", "return {}"),
        ]);
        let v2 = tgz(&[
            (
                "r-2/tuse-plugin.json",
                r#"{"name":"demo","version":"2.0.0"}"#,
            ),
            ("r-2/init.lua", "return {}"),
        ]);
        let bad = tgz(&[("r-bad/init.lua", "return {}")]);
        let bad_entry = tgz(&[
            (
                "r-esc/tuse-plugin.json",
                r#"{"name":"esc","version":"1.0.0","entrypoint":"../evil.lua"}"#,
            ),
            ("r-esc/init.lua", "return {}"),
        ]);
        let bad_ver = tgz(&[
            ("r-ver/tuse-plugin.json", r#"{"name":"bv","version":""}"#),
            ("r-ver/init.lua", "return {}"),
        ]);
        let bad_symlink = {
            let mut enc = GzEncoder::new(Vec::new(), Compression::default());
            {
                let mut tar = tar::Builder::new(&mut enc);
                let mut h = tar::Header::new_gnu();
                h.set_entry_type(tar::EntryType::Symlink);
                h.set_size(0);
                h.set_mode(0o777);
                h.set_cksum();
                tar.append_link(&mut h, "r-sym/link", "/etc/passwd")
                    .unwrap();
                tar.finish().unwrap();
            }
            enc.finish().unwrap()
        };
        let bad_trav = {
            let mut enc = GzEncoder::new(Vec::new(), Compression::default());
            {
                let mut tar = tar::Builder::new(&mut enc);
                let mut h = tar::Header::new_gnu();
                h.set_size(1);
                h.set_mode(0o644);
                h.set_path("r-tr/a").unwrap();
                let raw = h.as_gnu_mut().unwrap();
                let name_bytes = b"r-tr/../../escape.lua";
                raw.name[..name_bytes.len()].copy_from_slice(name_bytes);
                raw.name[name_bytes.len()] = 0;
                h.set_cksum();
                tar.append(&h, &b"x"[..]).unwrap();
                tar.finish().unwrap();
            }
            enc.finish().unwrap()
        };
        let url = serve(vec![
            ("/acme/demo.tar.gz".into(), good),
            ("/acme/demo/v2.tar.gz".into(), v2),
            ("/acme/bad.tar.gz".into(), bad),
            ("/acme/esc.tar.gz".into(), bad_entry),
            ("/acme/bv.tar.gz".into(), bad_ver),
            ("/acme/sym.tar.gz".into(), bad_symlink),
            ("/acme/trav.tar.gz".into(), bad_trav),
        ]);
        unsafe {
            std::env::set_var("TUSE_HOME", &home);
            std::env::set_var("TUSE_PLUGINS_URL", &url);
        }

        for src in ["acme/bad", "acme/esc", "acme/bv", "acme/sym", "acme/trav"] {
            assert!(
                Plugins::add(src, false).is_err(),
                "expected failure for {src}"
            );
        }
        assert!(Plugins::list().unwrap().is_empty());

        let local_dir = home.join("plugins/localonly");
        std::fs::create_dir_all(&local_dir).unwrap();
        std::fs::write(
            local_dir.join("tuse-plugin.json"),
            r#"{"name":"localonly","version":"0.1.0"}"#,
        )
        .unwrap();
        std::fs::write(local_dir.join("init.lua"), "return {}").unwrap();
        let local_err = Plugins::update(Some("localonly")).unwrap_err();
        assert!(local_err.contains("no recorded source"), "{local_err}");
        assert!(local_dir.join("init.lua").is_file());
        Plugins::delete("localonly").unwrap();

        let added = Plugins::add("acme/demo", false).unwrap();
        assert_eq!(
            (added.name.as_str(), added.version.as_str(), added.replaced),
            ("demo", "1.0.0", false)
        );
        assert!(Plugins::add("acme/demo", false).is_err());

        let replaced = Plugins::add("acme/demo@v2", true).unwrap();
        assert_eq!(
            (replaced.version.as_str(), replaced.replaced),
            ("2.0.0", true)
        );

        let sub = Plugins::add("https://github.com/acme/demo/tree/v2", true).unwrap();
        assert_eq!(sub.version, "2.0.0");

        let sub2 = Plugins::add("acme/demo/packs/sub", false).unwrap();
        assert_eq!(sub2.name, "sub");

        let list = Plugins::list().unwrap();
        assert_eq!(list.len(), 2);
        let detail = Plugins::get("demo").unwrap();
        assert_eq!(detail.manifest["version"], "2.0.0");
        assert!(detail.source.is_some());

        let updated = Plugins::update(Some("demo")).unwrap();
        assert_eq!(updated.len(), 1);
        let updated_all = Plugins::update(None).unwrap();
        assert_eq!(updated_all.len(), 2);

        assert!(Plugins::add("acme/bad", false).is_err());
        assert_eq!(Plugins::list().unwrap().len(), 2);

        assert_eq!(Plugins::delete("demo").unwrap(), "demo");
        assert!(Plugins::get("demo").is_err());
        assert_eq!(Plugins::list().unwrap().len(), 1);

        unsafe {
            std::env::remove_var("TUSE_HOME");
            std::env::remove_var("TUSE_PLUGINS_URL");
        }
        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn concurrent_discovery_and_mutations_stay_consistent_without_hiding_broken_manifests() {
        let _g = crate::storage::ENV_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let home = std::env::temp_dir().join(format!("tuse-plug-conc-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&home);
        let stable_v1 = tgz(&[
            (
                "s1/tuse-plugin.json",
                r#"{"name":"stable","version":"1.0.0"}"#,
            ),
            ("s1/init.lua", "return { prompts = { s = '1' } }"),
        ]);
        let stable_v2 = tgz(&[
            (
                "s2/tuse-plugin.json",
                r#"{"name":"stable","version":"2.0.0"}"#,
            ),
            ("s2/init.lua", "return { prompts = { s = '2' } }"),
        ]);
        let churn_v1 = tgz(&[
            (
                "c1/tuse-plugin.json",
                r#"{"name":"churn","version":"1.0.0"}"#,
            ),
            ("c1/init.lua", "return { prompts = { c = '1' } }"),
            ("c1/extra.lua", "return 1"),
        ]);
        let churn_v2 = tgz(&[
            (
                "c2/tuse-plugin.json",
                r#"{"name":"churn","version":"2.0.0"}"#,
            ),
            ("c2/init.lua", "return { prompts = { c = '2' } }"),
            ("c2/extra.lua", "return 2"),
        ]);
        let url = serve(vec![
            ("/acme/stable.tar.gz".into(), stable_v1),
            ("/acme/stable/v2.tar.gz".into(), stable_v2),
            ("/acme/churn.tar.gz".into(), churn_v1),
            ("/acme/churn/v2.tar.gz".into(), churn_v2),
        ]);
        unsafe {
            std::env::set_var("TUSE_HOME", &home);
            std::env::set_var("TUSE_PLUGINS_URL", &url);
        }

        Plugins::add("acme/stable", false).unwrap();

        let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let mut readers = vec![];
        for _ in 0..4 {
            let stop = stop.clone();
            readers.push(std::thread::spawn(move || {
                while !stop.load(Ordering::Relaxed) {
                    let disc = tuse_config::discover_plugins().unwrap();
                    assert!(
                        disc.iter().all(|p| !p.name.starts_with('.')),
                        "saw internal dir: {:?}",
                        disc.iter().map(|p| &p.name).collect::<Vec<_>>()
                    );
                    assert!(
                        disc.iter().any(|p| p.name == "stable"),
                        "stable vanished during concurrent replace"
                    );
                    let list = Plugins::list().unwrap();
                    assert!(list.iter().any(|p| p.name == "stable"));
                    let detail = Plugins::get("stable").unwrap();
                    assert!(matches!(detail.version.as_str(), "1.0.0" | "2.0.0"));
                }
            }));
        }

        let w1 = std::thread::spawn(|| {
            for i in 0..12 {
                let src = if i % 2 == 0 {
                    "acme/stable@v2"
                } else {
                    "acme/stable"
                };
                Plugins::add(src, true).unwrap();
            }
        });
        let w2 = std::thread::spawn(|| {
            for _ in 0..12 {
                Plugins::add("acme/churn", false).unwrap();
                Plugins::add("acme/churn@v2", true).unwrap();
                Plugins::update(Some("churn")).unwrap();
                Plugins::delete("churn").unwrap();
            }
        });

        w1.join().unwrap();
        w2.join().unwrap();
        stop.store(true, Ordering::Relaxed);
        for r in readers {
            r.join().unwrap();
        }

        let broken = plugins_dir().join("broken");
        std::fs::create_dir_all(&broken).unwrap();
        std::fs::write(broken.join("tuse-plugin.json"), r#"{"version":"1.0.0"}"#).unwrap();
        let err = tuse_config::discover_plugins().unwrap_err();
        assert!(
            err.contains("broken") && err.contains("entrypoint"),
            "{err}"
        );

        unsafe {
            std::env::remove_var("TUSE_HOME");
            std::env::remove_var("TUSE_PLUGINS_URL");
        }
        let _ = std::fs::remove_dir_all(&home);
    }
}
