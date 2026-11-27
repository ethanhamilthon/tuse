use crate::args::Args;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Component, Path};
use tuse_config::{InstalledPlugin, discover_plugins, plugins_dir, plugins_meta_path};

pub const USAGE: &str = "tuse plugins <add|list|delete|print|update>

USAGE
  tuse plugins add [--replace] REPO     install a plugin from a GitHub repo or subdirectory
  tuse plugins list [--json]            list installed plugins
  tuse plugins delete TARGET            remove an installed plugin
  tuse plugins print [--json] TARGET    print installed plugin details
  tuse plugins update [TARGET]          update one or all installed plugins";

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PluginMeta {
    source: String,
}

struct Repo {
    source: String,
    owner: String,
    repo: String,
    subdir: Option<String>,
    git_ref: Option<String>,
}

fn pe(p: &Path, e: impl std::fmt::Display) -> String {
    format!("{}: {e}", p.display())
}

pub fn run(rest: &[String]) -> Result<(), String> {
    let Some(first) = rest.first().map(String::as_str) else {
        return Err(format!("missing subcommand\n\n{USAGE}"));
    };
    if matches!(first, "-h" | "--help") {
        println!("{USAGE}");
        return Ok(());
    }
    let tail = &rest[1..];
    let (usage, plain, min, max): (&str, &[&str], usize, usize) = match first {
        "add" => ("tuse plugins add [--replace] REPO", &["--replace"], 1, 1),
        "list" => ("tuse plugins list [--json]", &["--json"], 0, 0),
        "delete" => ("tuse plugins delete TARGET", &[], 1, 1),
        "print" => ("tuse plugins print [--json] TARGET", &["--json"], 1, 1),
        "update" => ("tuse plugins update [TARGET]", &[], 0, 1),
        other => return Err(format!("unknown plugins subcommand `{other}`\n\n{USAGE}")),
    };
    if tail
        .iter()
        .take_while(|s| *s != "--")
        .any(|s| s == "-h" || s == "--help")
    {
        println!("{usage}");
        return Ok(());
    }
    let a = Args::parse(tail, &[], plain)?;
    if let Some(f) = a.unknown(plain) {
        return Err(format!("unknown flag {f}\nusage: {usage}"));
    }
    a.expect_args_range(min, max)
        .map_err(|m| format!("{m}\nusage: {usage}"))?;
    if first == "add" {
        return install(a.arg(0), a.has(&["--replace"]));
    }
    let (plugins, mut meta) = (discover_plugins()?, read_meta());
    let find = |target: &str| -> Result<&InstalledPlugin, String> {
        plugins
            .iter()
            .find(|p| p.name == target || meta.get(&p.name).is_some_and(|m| m.source == target))
            .ok_or_else(|| format!("plugin `{target}` is not installed"))
    };
    match first {
        "list" if a.has(&["--json"]) => {
            let rows: Vec<_> = plugins.iter().map(|p| pjson(p, &meta, false)).collect();
            println!(
                "{}",
                serde_json::to_string_pretty(&rows).map_err(|e| e.to_string())?
            );
        }
        "list" => {
            for p in &plugins {
                let s = meta.get(&p.name).map_or("(local)", |m| m.source.as_str());
                println!("{}\t{}\t{s}", p.name, p.manifest.version);
            }
        }
        "delete" => {
            let name = find(a.arg(0))?.name.clone();
            std::fs::remove_dir_all(plugins_dir().join(&name)).map_err(|e| e.to_string())?;
            meta.remove(&name);
            write_meta(&meta)?;
            println!("Removed plugin `{name}`");
        }
        "print" => {
            let p = find(a.arg(0))?;
            if a.has(&["--json"]) {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&pjson(p, &meta, true))
                        .map_err(|e| e.to_string())?
                );
            } else {
                let s = meta.get(&p.name).map_or("(local)", |m| m.source.as_str());
                let m = serde_json::to_string_pretty(&p.raw_manifest).map_err(|e| e.to_string())?;
                println!(
                    "name: {}\nversion: {}\nentrypoint: {}\npath: {}\nsource: {s}\nmanifest: {m}",
                    p.name,
                    p.manifest.version,
                    p.manifest.entrypoint,
                    p.dir.display()
                );
            }
        }
        "update" => {
            let targets: Vec<_> = match a.opt_arg(0) {
                Some(t) => vec![find(t)?],
                None if plugins.is_empty() => {
                    println!("No plugins installed.");
                    return Ok(());
                }
                None => plugins.iter().collect(),
            };
            let errs: Vec<_> = targets
                .into_iter()
                .filter_map(|p| match meta.get(&p.name) {
                    Some(m) => install(&m.source, true)
                        .err()
                        .map(|e| format!("{}: {e}", p.name)),
                    None => Some(format!("plugin `{}` has no recorded source", p.name)),
                })
                .collect();
            if !errs.is_empty() {
                return Err(errs.join("\n"));
            }
        }
        _ => unreachable!(),
    }
    Ok(())
}

fn pjson(p: &InstalledPlugin, meta: &BTreeMap<String, PluginMeta>, full: bool) -> Value {
    let mut v = json!({
        "name": p.name, "version": p.manifest.version, "entrypoint": p.manifest.entrypoint,
        "path": p.dir.display().to_string(), "source": meta.get(&p.name).map(|m| m.source.as_str()),
    });
    if full {
        v["manifest"] = p.raw_manifest.clone();
    }
    v
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

fn read_meta() -> BTreeMap<String, PluginMeta> {
    std::fs::read_to_string(plugins_meta_path())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}
fn write_meta(map: &BTreeMap<String, PluginMeta>) -> Result<(), String> {
    let path = plugins_meta_path();
    if let Some(p) = path.parent() {
        std::fs::create_dir_all(p).map_err(|e| pe(p, e))?;
    }
    let body = serde_json::to_string_pretty(map).map_err(|e| e.to_string())?;
    std::fs::write(&path, body).map_err(|e| pe(&path, e))
}
fn install(raw: &str, replace: bool) -> Result<(), String> {
    let r = parse_repo(raw)?;
    let url = match (
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
    };
    let mut bytes = vec![];
    ureq::get(&url)
        .set("User-Agent", concat!("tuse/", env!("CARGO_PKG_VERSION")))
        .call()
        .map_err(|e| format!("{url}: {e}"))?
        .into_reader()
        .read_to_end(&mut bytes)
        .map_err(|e| format!("{url}: {e}"))?;
    let root = plugins_dir();
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let staging = root.join(format!(".staging-{}-{stamp}", std::process::id()));
    std::fs::create_dir_all(&staging).map_err(|e| pe(&staging, e))?;
    let res = (|| -> Result<(String, String), String> {
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
        if dest.exists() {
            if !replace {
                return Err(format!(
                    "plugin `{name}` already exists; pass `--replace` or run `tuse plugins delete {name}`"
                ));
            }
            std::fs::remove_dir_all(&dest).map_err(|e| pe(&dest, e))?;
        }
        std::fs::rename(&staging, &dest).map_err(|e| pe(&dest, e))?;
        let mut meta = read_meta();
        meta.insert(name.clone(), PluginMeta { source: r.source });
        write_meta(&meta)?;
        Ok((name, parsed.manifest.version))
    })();
    if res.is_err() {
        let _ = std::fs::remove_dir_all(&staging);
    }
    let (name, ver) = res?;
    println!(
        "{} plugin `{name}` ({ver})",
        if replace { "Updated" } else { "Installed" }
    );
    Ok(())
}
