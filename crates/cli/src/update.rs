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
    let want = String::from_utf8_lossy(sums)
        .lines()
        .find(|l| l.ends_with(&format!(" {asset}")))
        .and_then(|l| l.split_whitespace().next().map(String::from))
        .ok_or("checksums.txt has no entry for this platform")?;
    if sha256(tarball) != want {
        return Err(format!("checksum mismatch for {asset}"));
    }
    write(&tmp.join("tuse"), &unpack(tarball, "tuse")?)?;
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

fn sha256(data: &[u8]) -> String {
    use sha2::Digest;
    sha2::Sha256::digest(data)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// The file `name` of a `.tar.gz`.
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
    #[test]
    fn checks_and_unpacks_a_release() {
        assert_eq!(
            super::sha256(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        let mut tar = tar::Builder::new(vec![]);
        let mut h = tar::Header::new_gnu();
        h.set_size(3);
        h.set_mode(0o755);
        h.set_cksum();
        tar.append_data(&mut h, "tuse", &b"bin"[..]).unwrap();
        let gz = flate2::write::GzEncoder::new(vec![], flate2::Compression::fast());
        let mut gz = gz;
        std::io::Write::write_all(&mut gz, &tar.into_inner().unwrap()).unwrap();
        let data = gz.finish().unwrap();
        assert_eq!(super::unpack(&data, "tuse").unwrap(), b"bin");
        assert!(super::unpack(&data, "other").is_err());
        let tmp = std::env::temp_dir().join(format!("tuse-unpack-{}", std::process::id()));
        let sums = format!("{}  x.tar.gz\n", super::sha256(b"nope"));
        let err = super::install(&tmp, "x.tar.gz", &data, sums.as_bytes()).unwrap_err();
        assert!(err.contains("checksum mismatch"), "{err}");
        std::fs::remove_dir_all(tmp).ok();
    }
}
