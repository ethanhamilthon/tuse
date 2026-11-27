use flate2::Compression;
use flate2::write::GzEncoder;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::path::Path;
use std::process::{Command, Output};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

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

fn serve(body: Vec<u8>) -> (String, Arc<AtomicUsize>) {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", l.local_addr().unwrap());
    let hits = Arc::new(AtomicUsize::new(0));
    let h = hits.clone();
    std::thread::spawn(move || {
        for mut s in l.incoming().flatten() {
            let mut r = BufReader::new(s.try_clone().unwrap());
            let mut line = String::new();
            while r.read_line(&mut line).unwrap_or(0) > 0 && !line.trim().is_empty() {
                line.clear();
            }
            h.fetch_add(1, Ordering::SeqCst);
            let hdr = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/gzip\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            let _ = s.write_all(hdr.as_bytes());
            let _ = s.write_all(&body);
        }
    });
    (url, hits)
}

fn tuse(dir: &Path, url: &str, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_tuse"))
        .current_dir(dir)
        .env("HOME", dir)
        .env("TUSE_PLUGINS_URL", url)
        .args(args)
        .output()
        .unwrap()
}

#[test]
fn plugins_add_list_print_update_delete_and_subdir() {
    let dir = std::env::temp_dir().join(format!("tuse-cli-plugins-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let good = tgz(&[
        (
            "repo-main/tuse-plugin.json",
            r#"{"name":"demo","version":"1.0.0"}"#,
        ),
        ("repo-main/init.lua", "return { prompts = { hi = 'ok' } }"),
        (
            "repo-main/packs/sub/tuse-plugin.json",
            r#"{"version":"0.2.0"}"#,
        ),
        ("repo-main/packs/sub/init.lua", "return {}"),
    ]);
    let (url, hits) = serve(good);

    let out = tuse(&dir, &url, &["plugins", "add", "acme/demo"]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        !tuse(&dir, &url, &["plugins", "add", "acme/demo"])
            .status
            .success()
    );
    assert!(
        tuse(&dir, &url, &["plugins", "add", "--replace", "acme/demo"])
            .status
            .success()
    );
    assert!(
        tuse(&dir, &url, &["plugins", "add", "acme/demo/packs/sub"])
            .status
            .success()
    );

    let list_json =
        String::from_utf8(tuse(&dir, &url, &["plugins", "list", "--json"]).stdout).unwrap();
    assert!(
        list_json.contains(r#""name": "demo""#) && list_json.contains(r#""name": "sub""#),
        "{list_json}"
    );
    let printed =
        String::from_utf8(tuse(&dir, &url, &["plugins", "print", "--json", "demo"]).stdout)
            .unwrap();
    assert!(
        printed.contains(r#""version": "1.0.0""#) && printed.contains(r#""manifest""#),
        "{printed}"
    );
    let before = hits.load(Ordering::SeqCst);
    assert!(
        tuse(&dir, &url, &["plugins", "update", "demo"])
            .status
            .success()
    );
    assert!(hits.load(Ordering::SeqCst) > before);
    assert!(
        tuse(&dir, &url, &["plugins", "delete", "demo"])
            .status
            .success()
    );
    assert!(!dir.join(".tuse/plugins/demo").exists());

    let bad = tgz(&[("repo-main/init.lua", "return {}")]);
    let (bad_url, _) = serve(bad);
    assert!(
        !tuse(&dir, &bad_url, &["plugins", "add", "acme/nomanifest"])
            .status
            .success()
    );
    let _ = std::fs::remove_dir_all(&dir);
}
