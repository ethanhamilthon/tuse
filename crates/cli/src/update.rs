use serde::{Deserialize, Serialize};
use std::io::{BufRead, Read, Write};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LastBackup {
    version: String,
    file: String,
}

fn env_nonempty(k: &str) -> Option<String> {
    std::env::var(k).ok().filter(|v| !v.trim().is_empty())
}

fn current_version() -> String {
    env_nonempty("TUSE_CURRENT_VERSION").unwrap_or_else(|| env!("CARGO_PKG_VERSION").to_string())
}

fn target_exe() -> Result<PathBuf, String> {
    match env_nonempty("TUSE_EXE_PATH") {
        Some(p) => Ok(PathBuf::from(p)),
        None => std::env::current_exe().map_err(|e| e.to_string()),
    }
}

fn base_url() -> String {
    env_nonempty("TUSE_RELEASE_URL")
        .unwrap_or_else(|| format!("{}/releases/latest/download", env!("CARGO_PKG_REPOSITORY")))
}

fn fetch_bytes(url: &str) -> Result<Vec<u8>, String> {
    let resp = ureq::get(url).call().map_err(|e| format!("{url}: {e}"))?;
    let mut buf = vec![];
    resp.into_reader()
        .read_to_end(&mut buf)
        .map_err(|e| format!("{url}: {e}"))?;
    Ok(buf)
}

fn fetch_text(url: &str) -> Option<String> {
    fetch_bytes(url)
        .ok()
        .and_then(|b| String::from_utf8(b).ok())
}

fn fetch_asset(name: &str) -> Result<Vec<u8>, String> {
    fetch_bytes(&format!("{}/{name}", base_url().trim_end_matches('/')))
}

fn parse_semver(s: &str) -> Option<(u64, u64, u64)> {
    let s = s.trim().strip_prefix('v').unwrap_or(s.trim());
    let mut it = s.split('.');
    let major = it.next()?.parse().ok()?;
    let minor = it.next()?.parse().ok()?;
    let patch_part = it.next()?;
    let patch_digits: String = patch_part
        .chars()
        .take_while(|c| c.is_ascii_digit())
        .collect();
    if patch_digits.is_empty() {
        return None;
    }
    Some((major, minor, patch_digits.parse().ok()?))
}

fn parse_changelog_sections(text: &str) -> Vec<(String, String)> {
    let mut out = vec![];
    let mut cur_ver: Option<String> = None;
    let mut cur_body = String::new();
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("## ") {
            let ver = rest.trim().strip_prefix('v').unwrap_or(rest.trim());
            if let Some(prev) = cur_ver.take() {
                out.push((prev, cur_body.trim_end().to_string()));
                cur_body.clear();
            }
            cur_ver = Some(ver.to_string());
            cur_body.push_str(line);
            cur_body.push('\n');
        } else if cur_ver.is_some() {
            cur_body.push_str(line);
            cur_body.push('\n');
        }
    }
    if let Some(prev) = cur_ver {
        out.push((prev, cur_body.trim_end().to_string()));
    }
    out
}

fn intervening_changelog(changelog: &str, old: &str, latest: &str) -> String {
    let (Some(old_v), Some(new_v)) = (parse_semver(old), parse_semver(latest)) else {
        return String::new();
    };
    let mut parts = vec![];
    for (ver, body) in parse_changelog_sections(changelog) {
        if let Some(v) = parse_semver(&ver)
            && v > old_v
            && v <= new_v
        {
            parts.push(body);
        }
    }
    parts.join("\n\n")
}

fn parse_release_tag(final_url: &str) -> Result<(String, String), String> {
    let tag = final_url
        .rsplit_once("/tag/")
        .map(|(_, t)| {
            t.split(['?', '#'])
                .next()
                .unwrap_or(t)
                .trim_end_matches('/')
        })
        .filter(|s| !s.is_empty())
        .ok_or_else(|| format!("could not determine latest release tag from {final_url}"))?;
    let latest = tag.strip_prefix('v').unwrap_or(tag).to_string();
    parse_semver(&latest).ok_or_else(|| format!("invalid release version `{latest}`"))?;
    Ok((tag.to_string(), latest))
}

fn discover_latest() -> Result<(String, String), String> {
    let changelog_override = env_nonempty("TUSE_CHANGELOG_URL");
    if let Some(base) = env_nonempty("TUSE_RELEASE_URL") {
        let base = base.trim_end_matches('/');
        let cl_url = changelog_override.unwrap_or_else(|| format!("{base}/CHANGELOG.md"));
        let changelog = fetch_text(&cl_url).unwrap_or_default();
        let latest = fetch_text(&format!("{base}/version"))
            .map(|s| s.trim().strip_prefix('v').unwrap_or(s.trim()).to_string())
            .filter(|s| !s.is_empty())
            .or_else(|| {
                parse_changelog_sections(&changelog)
                    .into_iter()
                    .next()
                    .map(|(v, _)| v)
            })
            .ok_or_else(|| format!("could not determine latest version from {base}"))?;
        parse_semver(&latest).ok_or_else(|| format!("invalid release version `{latest}`"))?;
        return Ok((latest, changelog));
    }
    let repo = env!("CARGO_PKG_REPOSITORY").trim_end_matches('/');
    let latest_url = format!("{repo}/releases/latest");
    let resp = ureq::get(&latest_url)
        .call()
        .map_err(|e| format!("{latest_url}: {e}"))?;
    let (tag, latest) = parse_release_tag(resp.get_url())?;
    let raw_repo = repo
        .strip_prefix("https://github.com/")
        .unwrap_or("ethanhamilthon/tuse");
    let cl_url = changelog_override.unwrap_or_else(|| {
        format!("https://raw.githubusercontent.com/{raw_repo}/{tag}/release/CHANGELOG.md")
    });
    let changelog = fetch_text(&cl_url)
        .or_else(|| {
            fetch_text(&format!(
                "https://raw.githubusercontent.com/{raw_repo}/main/release/CHANGELOG.md"
            ))
        })
        .unwrap_or_default();
    Ok((latest, changelog))
}

fn prompt_yes(
    prompt: &str,
    reader: &mut impl BufRead,
    writer: &mut impl Write,
) -> Result<bool, String> {
    write!(writer, "{prompt}").map_err(|e| e.to_string())?;
    writer.flush().map_err(|e| e.to_string())?;
    let mut line = String::new();
    let n = reader.read_line(&mut line).map_err(|e| e.to_string())?;
    if n == 0 {
        return Ok(false);
    }
    Ok(matches!(
        line.trim().to_ascii_lowercase().as_str(),
        "y" | "yes"
    ))
}

fn backups_dir() -> PathBuf {
    tuse_core::tuse_home().join("backups")
}

fn last_backup_path() -> PathBuf {
    backups_dir().join("last.json")
}

fn run_io(
    f: impl FnOnce(&mut std::io::StdinLock<'_>, &mut std::io::StdoutLock<'_>) -> Result<(), String>,
) -> Result<(), String> {
    let (stdin, stdout) = (std::io::stdin(), std::io::stdout());
    f(&mut stdin.lock(), &mut stdout.lock())
}

pub fn update() -> Result<(), String> {
    run_io(|r, w| update_io(r, w))
}

fn update_io(reader: &mut impl BufRead, writer: &mut impl Write) -> Result<(), String> {
    let old = current_version();
    let (latest, changelog) = discover_latest()?;
    writeln!(writer, "Installed: {old}  Latest: {latest}").map_err(|e| e.to_string())?;
    if let (Some(o), Some(l)) = (parse_semver(&old), parse_semver(&latest))
        && l <= o
    {
        writeln!(writer, "Already up to date.").map_err(|e| e.to_string())?;
        return Ok(());
    }
    let notes = intervening_changelog(&changelog, &old, &latest);
    if !notes.is_empty() && prompt_yes("Show changelog? [y/N] ", reader, writer)? {
        writeln!(writer, "\n{notes}\n").map_err(|e| e.to_string())?;
    }
    if !prompt_yes(
        &format!("Update tuse {old} -> {latest}? [y/N] "),
        reader,
        writer,
    )? {
        writeln!(writer, "Update cancelled.").map_err(|e| e.to_string())?;
        return Ok(());
    }
    let asset = format!("tuse_{}_{}.tar.gz", os()?, arch()?);
    writeln!(writer, "Downloading {asset}").map_err(|e| e.to_string())?;
    let (tarball, sums) = (fetch_asset(&asset)?, fetch_asset("checksums.txt")?);
    let backup = install_verified(&target_exe()?, &old, &asset, &tarball, &sums)?;
    writeln!(
        writer,
        "Updated {old} -> {latest} (backup: {})",
        backup.display()
    )
    .map_err(|e| e.to_string())
}

pub fn rollback() -> Result<(), String> {
    run_io(|r, w| rollback_io(r, w))
}

fn rollback_io(reader: &mut impl BufRead, writer: &mut impl Write) -> Result<(), String> {
    let meta_path = last_backup_path();
    let raw = std::fs::read_to_string(&meta_path)
        .map_err(|_| "no rollback backup available (run `tuse update` first)".to_string())?;
    let last: LastBackup = serde_json::from_str(&raw)
        .map_err(|e| format!("corrupt backup record {}: {e}", meta_path.display()))?;
    let backup_path = backups_dir().join(&last.file);
    if !backup_path.is_file() {
        return Err(format!(
            "rollback backup missing: {}",
            backup_path.display()
        ));
    }
    writeln!(
        writer,
        "Current: {}  Rollback target: {}",
        current_version(),
        last.version
    )
    .map_err(|e| e.to_string())?;
    if !prompt_yes(
        &format!("Restore tuse {} from backup? [y/N] ", last.version),
        reader,
        writer,
    )? {
        writeln!(writer, "Rollback cancelled.").map_err(|e| e.to_string())?;
        return Ok(());
    }
    atomic_copy(&backup_path, &target_exe()?, true)?;
    writeln!(writer, "Rolled back tuse to {}.", last.version).map_err(|e| e.to_string())
}

fn set_exec(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755));
    }
}

fn staged_path(dst: &Path) -> PathBuf {
    let name = dst.file_name().and_then(|n| n.to_str()).unwrap_or("tuse");
    dst.with_file_name(format!(".{name}.tmp.{}", std::process::id()))
}

fn finish_rename(staged: &Path, dst: &Path, exec: bool) -> Result<(), String> {
    if exec {
        set_exec(staged);
    }
    std::fs::rename(staged, dst).map_err(|e| {
        let _ = std::fs::remove_file(staged);
        format!("{}: {e}", dst.display())
    })
}

fn atomic_copy(src: &Path, dst: &Path, exec: bool) -> Result<(), String> {
    let staged = staged_path(dst);
    std::fs::copy(src, &staged).map_err(|e| format!("{}: {e}", staged.display()))?;
    finish_rename(&staged, dst, exec)
}

fn snapshot_file(path: &Path, dst: PathBuf, exec: bool) -> Result<Option<PathBuf>, String> {
    if !std::fs::symlink_metadata(path).is_ok_and(|m| m.is_file()) {
        return Ok(None);
    }
    std::fs::copy(path, &dst).map_err(|e| format!("{}: {e}", dst.display()))?;
    if exec {
        set_exec(&dst);
    }
    Ok(Some(dst))
}

fn restore_or_remove(prev: Option<PathBuf>, dst: &Path, exec: bool) {
    match prev {
        Some(p) => {
            let _ = finish_rename(&p, dst, exec);
        }
        None => {
            let _ = std::fs::remove_file(dst);
        }
    }
}

fn remove_files(paths: &[Option<&Path>]) {
    for p in paths.iter().flatten() {
        let _ = std::fs::remove_file(p);
    }
}

fn install_verified(
    exe: &Path,
    old_version: &str,
    asset: &str,
    tarball: &[u8],
    sums: &[u8],
) -> Result<PathBuf, String> {
    install_verified_with(exe, old_version, asset, tarball, sums, |s, e| {
        finish_rename(s, e, true)
    })
}

fn install_verified_with(
    exe: &Path,
    old_version: &str,
    asset: &str,
    tarball: &[u8],
    sums: &[u8],
    commit_exe: impl FnOnce(&Path, &Path) -> Result<(), String>,
) -> Result<PathBuf, String> {
    let want = String::from_utf8_lossy(sums)
        .lines()
        .find(|l| l.ends_with(&format!(" {asset}")))
        .and_then(|l| l.split_whitespace().next().map(String::from))
        .ok_or("checksums.txt has no entry for this platform")?;
    if sha256(tarball) != want {
        return Err(format!("checksum mismatch for {asset}"));
    }
    let bin = unpack(tarball, "tuse")?;
    let bdir = backups_dir();
    std::fs::create_dir_all(&bdir).map_err(|e| format!("{}: {e}", bdir.display()))?;

    let staged = staged_path(exe);
    std::fs::write(&staged, &bin).map_err(|e| format!("{}: {e}", staged.display()))?;
    set_exec(&staged);

    let backup_file = format!("{old_version}-tuse-bin");
    let backup_path = bdir.join(&backup_file);
    let backup_tmp = staged_path(&backup_path);
    if let Err(e) = std::fs::copy(exe, &backup_tmp) {
        remove_files(&[Some(&staged)]);
        return Err(format!("{}: {e}", backup_tmp.display()));
    }
    set_exec(&backup_tmp);

    let meta_path = last_backup_path();
    let meta = LastBackup {
        version: old_version.to_string(),
        file: backup_file.clone(),
    };
    let meta_json = serde_json::to_string_pretty(&meta).map_err(|e| {
        remove_files(&[Some(&staged), Some(&backup_tmp)]);
        e.to_string()
    })?;
    let meta_tmp = staged_path(&meta_path);
    if let Err(e) = std::fs::write(&meta_tmp, meta_json.as_bytes()) {
        remove_files(&[Some(&staged), Some(&backup_tmp)]);
        return Err(format!("{}: {e}", meta_tmp.display()));
    }

    let pid = std::process::id();
    let prev_bak = match snapshot_file(
        &backup_path,
        bdir.join(format!(".{backup_file}.prev.{pid}")),
        true,
    ) {
        Ok(p) => p,
        Err(e) => {
            remove_files(&[Some(&staged), Some(&backup_tmp), Some(&meta_tmp)]);
            return Err(e);
        }
    };
    let prev_meta = match snapshot_file(
        &meta_path,
        bdir.join(format!(".last.json.prev.{pid}")),
        false,
    ) {
        Ok(p) => p,
        Err(e) => {
            remove_files(&[
                Some(&staged),
                Some(&backup_tmp),
                Some(&meta_tmp),
                prev_bak.as_deref(),
            ]);
            return Err(e);
        }
    };

    if let Err(e) = finish_rename(&backup_tmp, &backup_path, true) {
        remove_files(&[
            Some(&staged),
            Some(&meta_tmp),
            prev_bak.as_deref(),
            prev_meta.as_deref(),
        ]);
        return Err(e);
    }

    if let Err(e) = finish_rename(&meta_tmp, &meta_path, false) {
        restore_or_remove(prev_bak, &backup_path, true);
        remove_files(&[Some(&staged), prev_meta.as_deref()]);
        return Err(e);
    }

    if let Err(e) = commit_exe(&staged, exe) {
        remove_files(&[Some(&staged)]);
        restore_or_remove(prev_meta, &meta_path, false);
        restore_or_remove(prev_bak, &backup_path, true);
        return Err(e);
    }

    remove_files(&[prev_bak.as_deref(), prev_meta.as_deref()]);
    Ok(backup_path)
}

fn sha256(data: &[u8]) -> String {
    use sha2::Digest;
    sha2::Sha256::digest(data)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn unpack(tarball: &[u8], name: &str) -> Result<Vec<u8>, String> {
    let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(tarball));
    for entry in archive.entries().map_err(|e| format!("archive: {e}"))? {
        let mut entry = entry.map_err(|e| format!("archive: {e}"))?;
        let path = entry.path().map_err(|e| format!("archive: {e}"))?;
        if path.as_os_str() == name {
            let mut out = vec![];
            entry
                .read_to_end(&mut out)
                .map_err(|e| format!("archive: {e}"))?;
            return Ok(out);
        }
    }
    Err(format!("archive has no `{name}`"))
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufReader, Cursor};
    use std::net::TcpListener;
    use std::sync::Mutex;

    static ENV: Mutex<()> = Mutex::new(());

    fn make_tarball(bin: &[u8]) -> Vec<u8> {
        let mut tar = tar::Builder::new(vec![]);
        let mut h = tar::Header::new_gnu();
        h.set_size(bin.len() as u64);
        h.set_mode(0o755);
        h.set_cksum();
        tar.append_data(&mut h, "tuse", bin).unwrap();
        let mut gz = flate2::write::GzEncoder::new(vec![], flate2::Compression::fast());
        gz.write_all(&tar.into_inner().unwrap()).unwrap();
        gz.finish().unwrap()
    }

    fn serve_files(files: Vec<(String, Vec<u8>)>) -> String {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", l.local_addr().unwrap());
        std::thread::spawn(move || {
            for mut s in l.incoming().flatten() {
                let mut r = BufReader::new(s.try_clone().unwrap());
                let mut first = String::new();
                let _ = r.read_line(&mut first);
                let mut line = String::new();
                while r.read_line(&mut line).unwrap_or(0) > 0 && !line.trim().is_empty() {
                    line.clear();
                }
                let path = first.split_whitespace().nth(1).unwrap_or("/");
                if let Some((_, body)) = files.iter().find(|(p, _)| p == path) {
                    let hdr = format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
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
    fn update_changelog_decline_bad_checksum_upgrade_and_non_semver_rollback() {
        let _lock = ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("tuse-update-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let home = dir.join("tuse-home");
        let exe = dir.join("tuse-fixture-bin");
        let cfg = dir.join("tuse.lua");
        std::fs::write(&exe, b"bin-0.14.3").unwrap();
        std::fs::write(&cfg, "return {}\n").unwrap();

        let asset = format!("tuse_{}_{}.tar.gz", os().unwrap(), arch().unwrap());
        let cl = "# Changelog\n\n## 0.15.0\n\n- big 0.15 change\n\n## 0.14.4\n\n- patch 0.14.4\n\n## 0.14.3\n\n- old note\n";
        let bin_015 = make_tarball(b"bin-0.15.0");
        let bin_016 = make_tarball(b"bin-0.16.0");
        let bad_sums = format!("{}  {asset}\n", "0".repeat(64));
        let good_sums_015 = format!("{}  {asset}\n", sha256(&bin_015));
        let good_sums_016 = format!("{}  {asset}\n", sha256(&bin_016));

        unsafe {
            std::env::set_var("TUSE_HOME", &home);
            std::env::set_var("TUSE_EXE_PATH", &exe);
            std::env::set_var("TUSE_CURRENT_VERSION", "0.14.3");
        }

        let mut out = Vec::new();
        assert!(rollback_io(&mut Cursor::new(b"y\n"), &mut out).is_err());

        assert_eq!(
            parse_release_tag("https://github.com/ethanhamilthon/tuse/releases/tag/v0.15.0")
                .unwrap()
                .1,
            "0.15.0"
        );
        for bad_url in [
            "https://github.com/ethanhamilthon/tuse/releases",
            "https://github.com/ethanhamilthon/tuse/releases/latest",
            "https://github.com/ethanhamilthon/tuse/releases/tag/nightly",
        ] {
            assert!(parse_release_tag(bad_url).is_err(), "{bad_url}");
        }

        let url_non_semver = serve_files(vec![("/version".into(), b"nightly\n".to_vec())]);
        unsafe {
            std::env::set_var("TUSE_RELEASE_URL", &url_non_semver);
        }
        let mut out = Vec::new();
        let err = update_io(&mut Cursor::new(b"y\n"), &mut out).unwrap_err();
        assert!(err.contains("invalid release version"), "{err}");

        let url_bad = serve_files(vec![
            ("/CHANGELOG.md".into(), cl.as_bytes().to_vec()),
            (format!("/{asset}"), bin_015.clone()),
            ("/checksums.txt".into(), bad_sums.into_bytes()),
        ]);
        unsafe {
            std::env::set_var("TUSE_RELEASE_URL", &url_bad);
        }
        let mut out = Vec::new();
        update_io(&mut Cursor::new(b"y\nn\n"), &mut out).unwrap();
        let text = String::from_utf8(out).unwrap();
        assert!(text.contains("Installed: 0.14.3  Latest: 0.15.0"), "{text}");
        assert!(text.contains("big 0.15 change") && text.contains("patch 0.14.4"));
        assert!(!text.contains("old note"));
        assert!(text.contains("Update cancelled."));
        assert_eq!(std::fs::read(&exe).unwrap(), b"bin-0.14.3");
        assert!(!home.join("backups/last.json").exists());

        let mut out = Vec::new();
        let err = update_io(&mut Cursor::new(b"n\ny\n"), &mut out).unwrap_err();
        assert!(err.contains("checksum mismatch"), "{err}");
        assert_eq!(std::fs::read(&exe).unwrap(), b"bin-0.14.3");
        assert!(!home.join("backups/last.json").exists());

        let url_015 = serve_files(vec![
            ("/CHANGELOG.md".into(), cl.as_bytes().to_vec()),
            (format!("/{asset}"), bin_015),
            ("/checksums.txt".into(), good_sums_015.into_bytes()),
        ]);
        unsafe {
            std::env::set_var("TUSE_RELEASE_URL", &url_015);
        }
        let mut out = Vec::new();
        update_io(&mut Cursor::new(b"n\ny\n"), &mut out).unwrap();
        assert_eq!(std::fs::read(&exe).unwrap(), b"bin-0.15.0");
        assert_eq!(
            std::fs::read(home.join("backups/0.14.3-tuse-bin")).unwrap(),
            b"bin-0.14.3"
        );

        std::fs::write(home.join("backups/9.9.9-tuse-bin"), b"bin-9.9.9").unwrap();

        let cl_016 = "# Changelog\n\n## 0.16.0\n\n- newer\n";
        let url_016 = serve_files(vec![
            ("/CHANGELOG.md".into(), cl_016.as_bytes().to_vec()),
            (format!("/{asset}"), bin_016.clone()),
            ("/checksums.txt".into(), good_sums_016.as_bytes().to_vec()),
        ]);
        unsafe {
            std::env::set_var("TUSE_CURRENT_VERSION", "0.14.9");
            std::env::set_var("TUSE_RELEASE_URL", &url_016);
        }
        let mut out = Vec::new();
        update_io(&mut Cursor::new(b"n\ny\n"), &mut out).unwrap();
        assert_eq!(std::fs::read(&exe).unwrap(), b"bin-0.16.0");

        let mut out = Vec::new();
        rollback_io(&mut Cursor::new(b"n\n"), &mut out).unwrap();
        assert_eq!(std::fs::read(&exe).unwrap(), b"bin-0.16.0");

        let mut out = Vec::new();
        rollback_io(&mut Cursor::new(b"y\n"), &mut out).unwrap();
        let rb_text = String::from_utf8(out).unwrap();
        assert!(rb_text.contains("0.14.9"), "{rb_text}");
        assert_eq!(std::fs::read(&exe).unwrap(), b"bin-0.15.0");
        assert_eq!(std::fs::read_to_string(&cfg).unwrap(), "return {}\n");

        let last_before = std::fs::read_to_string(home.join("backups/last.json")).unwrap();
        let bak_before = std::fs::read(home.join("backups/0.14.9-tuse-bin")).unwrap();
        let ro_dir = dir.join("readonly");
        std::fs::create_dir_all(&ro_dir).unwrap();
        let ro_exe = ro_dir.join("tuse");
        std::fs::write(&ro_exe, b"bin-ro").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&ro_dir, std::fs::Permissions::from_mode(0o555)).unwrap();
        }
        unsafe {
            std::env::set_var("TUSE_EXE_PATH", &ro_exe);
            std::env::set_var("TUSE_CURRENT_VERSION", "0.14.9");
            std::env::set_var("TUSE_RELEASE_URL", &url_016);
        }
        let mut out = Vec::new();
        assert!(update_io(&mut Cursor::new(b"n\ny\n"), &mut out).is_err());
        assert_eq!(
            std::fs::read_to_string(home.join("backups/last.json")).unwrap(),
            last_before
        );
        assert_eq!(
            std::fs::read(home.join("backups/0.14.9-tuse-bin")).unwrap(),
            bak_before
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&ro_dir, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        let dir_exe = dir.join("is-a-dir");
        std::fs::create_dir_all(&dir_exe).unwrap();
        unsafe {
            std::env::set_var("TUSE_EXE_PATH", &dir_exe);
        }
        let mut out = Vec::new();
        assert!(update_io(&mut Cursor::new(b"n\ny\n"), &mut out).is_err());
        assert_eq!(
            std::fs::read_to_string(home.join("backups/last.json")).unwrap(),
            last_before
        );
        assert_eq!(
            std::fs::read(home.join("backups/0.14.9-tuse-bin")).unwrap(),
            bak_before
        );

        let tx_home = dir.join("tx-home");
        let tx_bdir = tx_home.join("backups");
        let tx_exe = dir.join("tx-exe");
        std::fs::create_dir_all(&tx_bdir).unwrap();
        std::fs::write(&tx_exe, b"working-0.14.3").unwrap();
        let prev_bak_path = tx_bdir.join("0.14.2-tuse-bin");
        std::fs::write(&prev_bak_path, b"prev-valid-0.14.2").unwrap();
        let prev_last = serde_json::to_string_pretty(&LastBackup {
            version: "0.14.2".into(),
            file: "0.14.2-tuse-bin".into(),
        })
        .unwrap();
        std::fs::write(tx_bdir.join("last.json"), &prev_last).unwrap();
        unsafe {
            std::env::set_var("TUSE_HOME", &tx_home);
            std::env::set_var("TUSE_EXE_PATH", &tx_exe);
            std::env::set_var("TUSE_CURRENT_VERSION", "0.14.3");
        }

        let blocked_bak = tx_bdir.join("0.14.3-tuse-bin");
        std::fs::create_dir_all(&blocked_bak).unwrap();
        let err = install_verified(
            &tx_exe,
            "0.14.3",
            &asset,
            &bin_016,
            good_sums_016.as_bytes(),
        )
        .unwrap_err();
        assert!(!err.is_empty());
        assert_eq!(std::fs::read(&tx_exe).unwrap(), b"working-0.14.3");
        assert!(blocked_bak.is_dir());
        assert_eq!(std::fs::read(&prev_bak_path).unwrap(), b"prev-valid-0.14.2");
        assert_eq!(
            std::fs::read_to_string(tx_bdir.join("last.json")).unwrap(),
            prev_last
        );
        std::fs::remove_dir_all(&blocked_bak).unwrap();

        std::fs::write(&blocked_bak, b"existing-0.14.3-backup").unwrap();
        std::fs::remove_file(tx_bdir.join("last.json")).unwrap();
        std::fs::create_dir_all(tx_bdir.join("last.json")).unwrap();
        assert!(
            install_verified(
                &tx_exe,
                "0.14.3",
                &asset,
                &bin_016,
                good_sums_016.as_bytes()
            )
            .is_err()
        );
        assert_eq!(std::fs::read(&tx_exe).unwrap(), b"working-0.14.3");
        assert_eq!(
            std::fs::read(&blocked_bak).unwrap(),
            b"existing-0.14.3-backup"
        );
        assert!(tx_bdir.join("last.json").is_dir());

        std::fs::remove_file(&blocked_bak).unwrap();
        assert!(
            install_verified(
                &tx_exe,
                "0.14.3",
                &asset,
                &bin_016,
                good_sums_016.as_bytes()
            )
            .is_err()
        );
        assert_eq!(std::fs::read(&tx_exe).unwrap(), b"working-0.14.3");
        assert!(!blocked_bak.exists());
        assert!(tx_bdir.join("last.json").is_dir());

        std::fs::remove_dir_all(tx_bdir.join("last.json")).unwrap();
        std::fs::write(&blocked_bak, b"existing-0.14.3-backup").unwrap();
        std::fs::write(tx_bdir.join("last.json"), &prev_last).unwrap();
        assert!(
            install_verified_with(
                &tx_exe,
                "0.14.3",
                &asset,
                &bin_016,
                good_sums_016.as_bytes(),
                |_, _| Err("injected exe rename failure".into()),
            )
            .is_err()
        );
        assert_eq!(std::fs::read(&tx_exe).unwrap(), b"working-0.14.3");
        assert_eq!(
            std::fs::read(&blocked_bak).unwrap(),
            b"existing-0.14.3-backup"
        );
        assert_eq!(
            std::fs::read_to_string(tx_bdir.join("last.json")).unwrap(),
            prev_last
        );
        for entry in std::fs::read_dir(&tx_bdir).unwrap().flatten() {
            let n = entry.file_name().to_string_lossy().to_string();
            assert!(!n.starts_with('.'), "leftover temp file: {n}");
        }

        unsafe {
            std::env::remove_var("TUSE_HOME");
            std::env::remove_var("TUSE_EXE_PATH");
            std::env::remove_var("TUSE_CURRENT_VERSION");
            std::env::remove_var("TUSE_RELEASE_URL");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
