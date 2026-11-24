use std::fs::{File, OpenOptions, TryLockError};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

const TIMEOUT: Duration = Duration::from_secs(60);
static STAGE_SEQ: AtomicU64 = AtomicU64::new(0);

struct RepoSpec {
    name: String,
    url: String,
    root: PathBuf,
    git_ref: Option<String>,
}

struct GitOut {
    ok: bool,
    code: Option<i32>,
    stdout: String,
    stderr: String,
}

struct CleanDir(Option<PathBuf>);

impl Drop for CleanDir {
    fn drop(&mut self) {
        if let Some(p) = self.0.take() {
            let _ = std::fs::remove_dir_all(p);
        }
    }
}

pub(crate) fn sync_all(entries: &[crate::plugins::Entry]) -> Result<(), String> {
    let mut repos: Vec<RepoSpec> = vec![];
    for e in entries {
        let Some(repo) = e.plugin.repo() else {
            continue;
        };
        if let Some(existing) = repos.iter_mut().find(|r| r.root == repo.root()) {
            if existing.git_ref.is_none() && repo.git_ref().is_some() {
                existing.git_ref = repo.git_ref().map(String::from);
            }
            continue;
        }
        repos.push(RepoSpec {
            name: e.plugin.name().to_string(),
            url: repo.url().to_string(),
            root: repo.root().to_path_buf(),
            git_ref: repo.git_ref().map(String::from),
        });
    }
    for r in repos {
        sync_repo(&r.name, &r.url, &r.root, r.git_ref.as_deref())?;
    }
    Ok(())
}

fn sync_repo(name: &str, url: &str, root: &Path, git_ref: Option<&str>) -> Result<(), String> {
    let deadline = Instant::now() + TIMEOUT;
    let _lock = lock_repo(name, root, deadline)?;
    if !root.exists() {
        clone_repo(name, url, root, git_ref, deadline)
    } else {
        update_repo(name, url, root, git_ref, deadline)
    }
}

fn fnv1a_128(bytes: &[u8]) -> u128 {
    let mut h: u128 = 0x6c62_272e_07bb_0142_62b8_2175_6295_c58d;
    for &b in bytes {
        h ^= u128::from(b);
        h = h.wrapping_mul(0x0000_0000_0100_0000_0000_0000_0000_013b);
    }
    h
}

fn lock_repo(name: &str, root: &Path, deadline: Instant) -> Result<File, String> {
    let dir = tuse_config::plugins_root().join(".locks");
    std::fs::create_dir_all(&dir).map_err(|e| format!("plugin {name}: {e}"))?;
    let hash = fnv1a_128(root.as_os_str().as_encoded_bytes());
    let path = dir.join(format!("{hash:032x}.lock"));
    let file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(&path)
        .map_err(|e| format!("plugin {name}: {e}"))?;
    loop {
        match file.try_lock() {
            Ok(()) => return Ok(file),
            Err(TryLockError::WouldBlock) => {
                if Instant::now() >= deadline {
                    return Err(format!(
                        "plugin {name}: timed out waiting for plugin repository lock"
                    ));
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            Err(TryLockError::Error(e)) => return Err(format!("plugin {name}: {e}")),
        }
    }
}

fn clone_repo(
    name: &str,
    url: &str,
    root: &Path,
    git_ref: Option<&str>,
    deadline: Instant,
) -> Result<(), String> {
    let parent = root.parent().unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(parent).map_err(|e| format!("plugin {name}: {e}"))?;
    let seq = STAGE_SEQ.fetch_add(1, Ordering::Relaxed);
    let base = root.file_name().and_then(|s| s.to_str()).unwrap_or("repo");
    let staging = parent.join(format!(".tmp-{base}-{}-{seq}", std::process::id()));
    let mut guard = CleanDir(Some(staging.clone()));
    let staging_str = staging.to_string_lossy().into_owned();
    git_ok(
        None,
        &["clone", "--no-checkout", url, &staging_str],
        name,
        "clone",
        deadline,
    )?;
    let target = resolve_ref(&staging, name, git_ref, deadline)?;
    git_ok(
        Some(&staging),
        &["checkout", "--detach", &target],
        name,
        "checkout",
        deadline,
    )?;
    std::fs::rename(&staging, root).map_err(|e| format!("plugin {name}: {e}"))?;
    guard.0 = None;
    Ok(())
}

fn update_repo(
    name: &str,
    url: &str,
    root: &Path,
    git_ref: Option<&str>,
    deadline: Instant,
) -> Result<(), String> {
    verify_git_repo(root, name, deadline)?;
    let origin = run_git(
        Some(root),
        &["config", "--get", "remote.origin.url"],
        name,
        "config",
        deadline,
    )?;
    let actual = if origin.ok { origin.stdout.trim() } else { "" };
    if tuse_config::normalize_url(actual) != tuse_config::normalize_url(url) {
        return Err(format!(
            "plugin {name}: checkout remote URL '{actual}' does not match '{url}'"
        ));
    }
    let head = rev_parse_commit(root, "HEAD", name, deadline)?;
    if let (Some(r), Some(h)) = (git_ref, head.as_deref())
        && is_pin_match(root, r, h, name, deadline)?
    {
        return Ok(());
    }
    git_ok(
        Some(root),
        &["fetch", "--prune", "--tags", "--force", "origin"],
        name,
        "fetch",
        deadline,
    )?;
    if git_ref.is_none() {
        git_ok(
            Some(root),
            &["remote", "set-head", "origin", "-a"],
            name,
            "remote",
            deadline,
        )?;
    }
    let target = resolve_ref(root, name, git_ref, deadline)?;
    if head.as_deref() == Some(target.as_str()) {
        return Ok(());
    }
    check_safe(root, name, head.is_some(), deadline)?;
    git_ok(
        Some(root),
        &["checkout", "--detach", "--no-overwrite-ignore", &target],
        name,
        "checkout",
        deadline,
    )?;
    Ok(())
}

fn verify_git_repo(root: &Path, name: &str, deadline: Instant) -> Result<(), String> {
    let bad = || {
        format!(
            "plugin {name}: {} exists and is not a git repository",
            root.display()
        )
    };
    if !root.join(".git").exists() {
        return Err(bad());
    }
    let top = run_git(
        Some(root),
        &["rev-parse", "--show-toplevel"],
        name,
        "rev-parse",
        deadline,
    )?;
    if !top.ok {
        return Err(bad());
    }
    let a = PathBuf::from(top.stdout.trim()).canonicalize().ok();
    let b = root.canonicalize().ok();
    if a.is_none() || a != b {
        return Err(bad());
    }
    Ok(())
}

fn is_hex_sha(s: &str) -> bool {
    (7..=40).contains(&s.len()) && s.bytes().all(|b| b.is_ascii_hexdigit())
}

fn is_pin_match(
    root: &Path,
    r: &str,
    head: &str,
    name: &str,
    deadline: Instant,
) -> Result<bool, String> {
    if !is_hex_sha(r) {
        return Ok(false);
    }
    if r.len() == 40 {
        return Ok(head.eq_ignore_ascii_case(r));
    }
    let tag = format!("refs/tags/{r}");
    let branch = format!("refs/remotes/origin/{r}");
    if show_ref(root, &tag, name, deadline)? || show_ref(root, &branch, name, deadline)? {
        return Ok(false);
    }
    Ok(rev_parse_commit(root, r, name, deadline)?.as_deref() == Some(head))
}

fn resolve_ref(
    root: &Path,
    name: &str,
    git_ref: Option<&str>,
    deadline: Instant,
) -> Result<String, String> {
    let Some(r) = git_ref else {
        return rev_parse_commit(root, "refs/remotes/origin/HEAD", name, deadline)?
            .ok_or_else(|| format!("plugin {name}: ref 'HEAD' not found"));
    };
    if !r.is_empty() {
        if let Some(sha) = rev_parse_commit(root, &format!("refs/tags/{r}"), name, deadline)? {
            return Ok(sha);
        }
        if let Some(sha) =
            rev_parse_commit(root, &format!("refs/remotes/origin/{r}"), name, deadline)?
        {
            return Ok(sha);
        }
        if is_hex_sha(r)
            && let Some(sha) = rev_parse_commit(root, r, name, deadline)?
        {
            return Ok(sha);
        }
    }
    Err(format!("plugin {name}: ref '{r}' not found"))
}

fn check_safe(root: &Path, name: &str, has_head: bool, deadline: Instant) -> Result<(), String> {
    let dirty = || {
        format!(
            "plugin {name}: checkout has local changes or unpublished commits; commit, push, stash, or discard them before updating"
        )
    };
    let status = run_git(
        Some(root),
        &["status", "--porcelain=v1", "--untracked-files=normal"],
        name,
        "status",
        deadline,
    )?;
    if !status.ok || !status.stdout.trim().is_empty() {
        return Err(dirty());
    }
    let gd = run_git(
        Some(root),
        &["rev-parse", "--git-dir"],
        name,
        "rev-parse",
        deadline,
    )?;
    if !gd.ok {
        return Err(dirty());
    }
    let git_dir = {
        let p = PathBuf::from(gd.stdout.trim());
        if p.is_absolute() { p } else { root.join(p) }
    };
    for marker in [
        "MERGE_HEAD",
        "REBASE_HEAD",
        "rebase-merge",
        "rebase-apply",
        "CHERRY_PICK_HEAD",
        "REVERT_HEAD",
        "BISECT_LOG",
    ] {
        if git_dir.join(marker).exists() {
            return Err(dirty());
        }
    }
    if has_head {
        let unpub = run_git(
            Some(root),
            &[
                "rev-list",
                "--count",
                "HEAD",
                "--not",
                "--remotes=origin",
                "--tags",
            ],
            name,
            "rev-list",
            deadline,
        )?;
        if !unpub.ok || unpub.stdout.trim() != "0" {
            return Err(dirty());
        }
    }
    Ok(())
}

fn show_ref(root: &Path, full: &str, name: &str, deadline: Instant) -> Result<bool, String> {
    let out = run_git(
        Some(root),
        &["show-ref", "--verify", "--quiet", full],
        name,
        "show-ref",
        deadline,
    )?;
    Ok(out.ok)
}

fn rev_parse_commit(
    root: &Path,
    spec: &str,
    name: &str,
    deadline: Instant,
) -> Result<Option<String>, String> {
    let arg = format!("{spec}^{{commit}}");
    let out = run_git(
        Some(root),
        &["rev-parse", "--verify", "--quiet", &arg],
        name,
        "rev-parse",
        deadline,
    )?;
    let s = out.stdout.trim();
    Ok((out.ok && !s.is_empty()).then(|| s.to_string()))
}

fn git_ok(
    cwd: Option<&Path>,
    args: &[&str],
    name: &str,
    op: &str,
    deadline: Instant,
) -> Result<String, String> {
    let out = run_git(cwd, args, name, op, deadline)?;
    if out.ok {
        return Ok(out.stdout);
    }
    let summary = out
        .stderr
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join("; ");
    let detail = if !summary.is_empty() {
        summary
    } else if let Some(code) = out.code {
        format!("exit code {code}")
    } else {
        "terminated".into()
    };
    Err(format!("plugin {name}: git {op} failed: {detail}"))
}

fn run_git(
    cwd: Option<&Path>,
    args: &[&str],
    name: &str,
    op: &str,
    deadline: Instant,
) -> Result<GitOut, String> {
    if Instant::now() >= deadline {
        return Err(format!("plugin {name}: git {op} failed: timed out"));
    }
    let mut c = Command::new("git");
    if let Some(dir) = cwd {
        c.current_dir(dir);
    }
    c.args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GCM_INTERACTIVE", "never");
    if std::env::var_os("GIT_ASKPASS").is_none() {
        c.env("GIT_ASKPASS", "/usr/bin/true");
    }
    if std::env::var_os("GIT_SSH_COMMAND").is_none() {
        c.env("GIT_SSH_COMMAND", "ssh -o BatchMode=yes");
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        c.process_group(0);
    }
    let mut child = c.spawn().map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            format!("plugin {name}: git is required to sync plugins")
        } else {
            format!("plugin {name}: git {op} failed: {e}")
        }
    })?;
    let mut out_pipe = child.stdout.take().unwrap();
    let mut err_pipe = child.stderr.take().unwrap();
    let t_out = std::thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = out_pipe.read_to_end(&mut buf);
        String::from_utf8_lossy(&buf).into_owned()
    });
    let t_err = std::thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = err_pipe.read_to_end(&mut buf);
        String::from_utf8_lossy(&buf).into_owned()
    });
    let status = loop {
        match child.try_wait() {
            Ok(Some(s)) => break s,
            Ok(None) if Instant::now() >= deadline => {
                tuse_lua::kill_group(child.id());
                let _ = child.wait();
                let _ = t_out.join();
                let _ = t_err.join();
                return Err(format!("plugin {name}: git {op} failed: timed out"));
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(10)),
            Err(e) => return Err(format!("plugin {name}: git {op} failed: {e}")),
        }
    };
    let stdout = t_out.join().unwrap_or_default();
    let stderr = t_err.join().unwrap_or_default();
    Ok(GitOut {
        ok: status.success(),
        code: status.code(),
        stdout,
        stderr,
    })
}
