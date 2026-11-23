//! `tuse plugins`: list, add, remove, install, update and clean the plugin clones.

use crate::args::Args;
use crate::plugins_edit as edit;
use std::path::{Path, PathBuf};
use std::process::Command;
use tuse_config::Plugin;

pub const USAGE: &str = "usage: tuse plugins [list | add NAME [--ref REF] | remove NAME | install | update [NAME] | clean] [-c|--config PATH]";

pub fn run(rest: &[String]) -> Result<(), String> {
    let a = Args::parse(rest, &["--config", "-c", "--ref"])?;
    if let Some(f) = a.unknown(&["--config", "-c", "--ref"]) {
        return Err(format!("unknown flag {f}\n{USAGE}"));
    }
    let cwd = std::env::current_dir().map_err(|e| e.to_string())?;
    let config = a.value(&["--config", "-c"]).map(|p| cwd.join(p));
    let pos: Vec<&str> = a.positional.iter().map(String::as_str).collect();
    let r#ref = a.value(&["--ref"]);
    match pos[..] {
        [] | ["list"] => list(&cwd, config.as_deref()),
        ["add", name] => add(&cwd, config.as_deref(), name, r#ref.as_deref()),
        ["remove", name] => remove(&cwd, config.as_deref(), name),
        ["install"] => sync(&cwd, config.as_deref(), None, false),
        ["update"] => sync(&cwd, config.as_deref(), None, true),
        ["update", name] => sync(&cwd, config.as_deref(), Some(name), true),
        ["clean"] => clean(&cwd, config.as_deref()),
        _ => Err(USAGE.into()),
    }
}

fn plugins(cwd: &Path, config: Option<&Path>) -> Result<(PathBuf, Vec<Plugin>), String> {
    tuse_core::configured_plugins(cwd, config)?.ok_or_else(|| "no config file found".into())
}

fn list(cwd: &Path, config: Option<&Path>) -> Result<(), String> {
    let (path, list) = plugins(cwd, config)?;
    println!("{}", path.display());
    for p in &list {
        let state = match (p.entry(), p.root.exists()) {
            (Some(_), _) => head(&p.root).unwrap_or_else(|| "local".into()),
            (None, true) => "no init.lua".into(),
            (None, false) => "not installed".into(),
        };
        let r = p
            .git_ref
            .as_deref()
            .map(|r| format!(" @{r}"))
            .unwrap_or_default();
        println!("  {}{r}  {state}", p.name);
    }
    if list.is_empty() {
        println!("  no plugins");
    }
    Ok(())
}

fn git(dir: Option<&Path>, args: &[&str]) -> Result<String, String> {
    let mut c = Command::new("git");
    if let Some(d) = dir {
        c.arg("-C").arg(d);
    }
    let out = c.args(args).output().map_err(|e| format!("git: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "git {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

fn head(root: &Path) -> Option<String> {
    root.join(".git")
        .exists()
        .then(|| git(Some(root), &["rev-parse", "--short", "HEAD"]).ok())?
}

/// Clones the repository when missing, then checks out `ref`; `pull` also fetches news.
fn fetch(p: &Plugin, pull: bool) -> Result<(), String> {
    let Some(url) = &p.url else { return Ok(()) };
    if !p.root.exists() {
        println!("clone {url}");
        std::fs::create_dir_all(p.root.parent().unwrap_or(&p.root)).map_err(|e| e.to_string())?;
        git(
            None,
            &["clone", "--quiet", url, &p.root.display().to_string()],
        )?;
    } else if pull {
        println!("fetch {url}");
        git(Some(&p.root), &["fetch", "--quiet", "--tags", "origin"])?;
    }
    if let Some(r) = &p.git_ref {
        git(Some(&p.root), &["checkout", "--quiet", r])?;
    }
    if pull && git(Some(&p.root), &["symbolic-ref", "-q", "HEAD"]).is_ok() {
        git(Some(&p.root), &["pull", "--quiet", "--ff-only"])?;
    }
    Ok(())
}

fn sync(cwd: &Path, config: Option<&Path>, only: Option<&str>, pull: bool) -> Result<(), String> {
    let (_, list) = plugins(cwd, config)?;
    let mut done: Vec<PathBuf> = vec![];
    for p in list.iter().filter(|p| only.is_none_or(|n| n == p.name)) {
        if done.contains(&p.root) {
            continue;
        }
        done.push(p.root.clone());
        fetch(p, pull).map_err(|e| format!("{}: {e}", p.name))?;
    }
    if let Some(n) = only.filter(|_| done.is_empty()) {
        return Err(format!("{n} is not in the config"));
    }
    println!("{} repositories up to date", done.len());
    Ok(())
}

/// The config `tuse` loads; `~/.config/tuse/init.lua` is created when there is none.
fn target(cwd: &Path, config: Option<&Path>) -> PathBuf {
    tuse_config::find(config, cwd)
        .unwrap_or_else(|| tuse_config::home().join(".config/tuse/init.lua"))
}

fn add(cwd: &Path, config: Option<&Path>, name: &str, r#ref: Option<&str>) -> Result<(), String> {
    let path = target(cwd, config);
    let base = path.parent().unwrap_or(cwd);
    let mut p = Plugin::new(name, None, None, base)?;
    p.git_ref = r#ref.map(str::to_string);
    fetch(&p, false)?;
    if p.entry().is_none() {
        return Err(format!(
            "{name}: no init.lua or tuse.lua in {}",
            p.dir().display()
        ));
    }
    let item = edit::entry(name, r#ref);
    let text = std::fs::read_to_string(&path).unwrap_or_default();
    if edit::has(&text, name) {
        println!("{name} is already in {}", path.display());
        return Ok(());
    }
    let new = if text.trim().is_empty() {
        Some(format!(
            "return {{\n    plugins = {{\n        {item},\n    }},\n}}\n"
        ))
    } else {
        edit::add(&text, &item)
    };
    let Some(new) = new else {
        println!(
            "installed {name}; add it to {} by hand:\n\n  g.plugins = g.plugins or {{}}\n  table.insert(g.plugins, {item})",
            path.display()
        );
        return Ok(());
    };
    std::fs::create_dir_all(base).map_err(|e| e.to_string())?;
    std::fs::write(&path, new).map_err(|e| format!("{}: {e}", path.display()))?;
    println!("added {name} to {}", path.display());
    Ok(())
}

fn remove(cwd: &Path, config: Option<&Path>, name: &str) -> Result<(), String> {
    let path = target(cwd, config);
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    match edit::remove(&text, name) {
        Some(new) => {
            std::fs::write(&path, new).map_err(|e| format!("{}: {e}", path.display()))?;
            println!(
                "removed {name} from {}; `tuse plugins clean` deletes unused clones",
                path.display()
            );
        }
        None => println!(
            "couldn't find a one-line entry for {name} in {}; remove it by hand",
            path.display()
        ),
    }
    Ok(())
}

fn clones(dir: &Path, out: &mut Vec<PathBuf>) {
    if dir.join(".git").exists() {
        out.push(dir.to_path_buf());
        return;
    }
    for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        if e.path().is_dir() {
            clones(&e.path(), out);
        }
    }
}

fn clean(cwd: &Path, config: Option<&Path>) -> Result<(), String> {
    let (_, list) = plugins(cwd, config)?;
    let mut all = vec![];
    clones(&tuse_config::plugins_root(), &mut all);
    for dir in all
        .into_iter()
        .filter(|d| !list.iter().any(|p| &p.root == d))
    {
        std::fs::remove_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        println!("removed {}", dir.display());
    }
    Ok(())
}
