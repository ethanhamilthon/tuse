use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;

fn base() -> String {
    std::env::var("TUSE_RELEASE_URL")
        .ok()
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| format!("{}/releases/latest/download", env!("CARGO_PKG_REPOSITORY")))
}

fn get(name: &str) -> Result<Vec<u8>, String> {
    let url = format!("{}/{name}", base().trim_end_matches('/'));
    let resp = ureq::get(&url).call().map_err(|e| format!("{url}: {e}"))?;
    let mut buf = vec![];
    resp.into_reader()
        .read_to_end(&mut buf)
        .map_err(|e| format!("{url}: {e}"))?;
    Ok(buf)
}

fn write(path: &Path, data: &[u8]) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    std::fs::write(path, data).map_err(|e| format!("{}: {e}", path.display()))
}

/// The folder of the global config and the config file itself.
fn global() -> Result<(PathBuf, PathBuf), String> {
    let file = tuse_config::global_path();
    let dir = file.parent().filter(|d| !d.as_os_str().is_empty());
    match dir {
        Some(d) if d != tuse_config::home() && d.parent().is_some() => {
            Ok((d.to_path_buf(), file.clone()))
        }
        _ => Err(format!(
            "refusing to reset {}: its folder is the home or the root",
            file.display()
        )),
    }
}

/// `tuse reset`: replaces the global config folder with the latest `main.yaml`.
pub fn reset(yes: bool) -> Result<(), String> {
    let (dir, file) = global()?;
    let main = get("main.yaml")?;
    if dir.exists() {
        if !yes && !confirm(&dir) {
            return Err("cancelled".into());
        }
        std::fs::remove_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    write(&file, &main)?;
    println!("Config reset in {}", dir.display());
    Ok(())
}

fn confirm(dir: &Path) -> bool {
    eprint!(
        "Delete {} and install fresh config files? [y/N] ",
        dir.display()
    );
    let mut answer = String::new();
    std::io::stdin().read_line(&mut answer).is_ok() && answer.trim().eq_ignore_ascii_case("y")
}

/// `tuse update`: replaces this binary with the latest release.
pub fn update() -> Result<(), String> {
    let old = env!("CARGO_PKG_VERSION");
    let asset = format!("tuse_{}_{}.tar.gz", os()?, arch()?);
    println!("Downloading {asset}");
    let (tarball, sums) = (get(&asset)?, get("checksums.txt")?);
    let tmp = std::env::temp_dir().join(format!("tuse-update-{}", std::process::id()));
    let result = install(&tmp, &asset, &tarball, &sums);
    let _ = std::fs::remove_dir_all(&tmp);
    let exe = result?;
    match run(Command::new(&exe).arg("--version")) {
        Ok(out) => {
            let trimmed = out.trim();
            let new = trimmed.strip_prefix("tuse ").unwrap_or(trimmed);
            if new.is_empty() {
                println!("Updated tuse");
            } else {
                println!("{old} -> {new}");
            }
        }
        Err(_) => println!("Updated tuse"),
    }
    Ok(())
}

fn install(tmp: &Path, asset: &str, tarball: &[u8], sums: &[u8]) -> Result<PathBuf, String> {
    let archive = tmp.join(asset);
    write(&archive, tarball)?;
    let want = String::from_utf8_lossy(sums)
        .lines()
        .find(|l| l.ends_with(&format!(" {asset}")))
        .and_then(|l| l.split_whitespace().next().map(String::from))
        .ok_or("checksums.txt has no entry for this platform")?;
    if sha256(&archive)? != want {
        return Err(format!("checksum mismatch for {asset}"));
    }
    run(Command::new("tar")
        .arg("-xzf")
        .arg(&archive)
        .arg("-C")
        .arg(tmp)
        .arg("tuse"))?;
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let staged = exe.with_extension("new");
    std::fs::copy(tmp.join("tuse"), &staged).map_err(|e| format!("{}: {e}", staged.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&staged, std::fs::Permissions::from_mode(0o755));
    }
    std::fs::rename(&staged, &exe).map_err(|e| format!("{}: {e}", exe.display()))?;
    Ok(exe)
}

fn run(cmd: &mut Command) -> Result<String, String> {
    let out = cmd.output().map_err(|e| format!("{cmd:?}: {e}"))?;
    if !out.status.success() {
        return Err(format!("{cmd:?}: {}", String::from_utf8_lossy(&out.stderr)));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

fn sha256(path: &Path) -> Result<String, String> {
    let out = run(Command::new("sha256sum").arg(path))
        .or_else(|_| run(Command::new("shasum").args(["-a", "256"]).arg(path)))?;
    out.split_whitespace()
        .next()
        .map(String::from)
        .ok_or_else(|| "no checksum tool output".into())
}

fn os() -> Result<&'static str, String> {
    match std::env::consts::OS {
        "macos" => Ok("darwin"),
        "linux" => Ok("linux"),
        other => Err(format!("unsupported OS {other}")),
    }
}

fn arch() -> Result<&'static str, String> {
    match std::env::consts::ARCH {
        "aarch64" => Ok("arm64"),
        "x86_64" => Ok("amd64"),
        other => Err(format!("unsupported CPU {other}")),
    }
}
