use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::process::{Command, Output};
use std::sync::{Arc, Mutex};

fn tuse(dir: &std::path::Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_tuse"))
        .current_dir(dir)
        .env("HOME", dir)
        .env("TEST_API_KEY", "secret-val")
        .args(args)
        .output()
        .unwrap()
}

type Seen = Arc<Mutex<Vec<(String, Vec<String>)>>>;

fn mock_models(body: &'static str) -> (String, Seen) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let seen = Arc::new(Mutex::new(vec![]));
    let log = seen.clone();
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let mut r = BufReader::new(stream.try_clone().unwrap());
            let mut first = String::new();
            let _ = r.read_line(&mut first);
            let mut headers = vec![];
            loop {
                let mut line = String::new();
                if r.read_line(&mut line).unwrap_or(0) == 0 || line.trim().is_empty() {
                    break;
                }
                headers.push(line.trim().to_ascii_lowercase());
            }
            log.lock()
                .unwrap()
                .push((first.trim().to_string(), headers));
            let mut out = stream;
            let _ = write!(
                out,
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
        }
    });
    (url, seen)
}

#[test]
fn connect_supports_four_apis_and_providers_keeps_two_columns_plus_json() {
    let dir = std::env::temp_dir().join(format!("tuse-cli-connect-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let (oai_url, oai_seen) = mock_models(r#"{"data":[{"id":"gpt-5"}]}"#);
    let (anth_url, anth_seen) = mock_models(r#"{"data":[{"id":"claude-4"}],"has_more":false}"#);
    let (gem_url, gem_seen) = mock_models(
        r#"{"models":[{"name":"models/gemini-2.5-pro","supportedGenerationMethods":["generateContent"]}]}"#,
    );

    let bad = tuse(
        &dir,
        &[
            "connect",
            "--base",
            &oai_url,
            "--key",
            "$TEST_API_KEY",
            "--name",
            "bad",
            "--api",
            "bogus",
        ],
    );
    assert!(!bad.status.success());
    let err = String::from_utf8(bad.stderr).unwrap();
    assert!(
        err.contains("openai-chat") && err.contains("google-gemini"),
        "{err}"
    );
    assert!(
        oai_seen.lock().unwrap().is_empty(),
        "invalid --api must not hit network"
    );

    let ok1 = tuse(
        &dir,
        &[
            "connect",
            "--base",
            &oai_url,
            "--key",
            "$TEST_API_KEY",
            "--name",
            "oai",
        ],
    );
    assert!(
        ok1.status.success(),
        "{}",
        String::from_utf8_lossy(&ok1.stderr)
    );

    let ok2 = tuse(
        &dir,
        &[
            "connect",
            "--base",
            &oai_url,
            "--key",
            "$TEST_API_KEY",
            "--name",
            "oai-r",
            "--api",
            "openai-responses",
        ],
    );
    assert!(
        ok2.status.success(),
        "{}",
        String::from_utf8_lossy(&ok2.stderr)
    );

    let ok3 = tuse(
        &dir,
        &[
            "connect",
            "--base",
            &anth_url,
            "--key",
            "$TEST_API_KEY",
            "--name",
            "anth",
            "--api",
            "anthropic-messages",
        ],
    );
    assert!(
        ok3.status.success(),
        "{}",
        String::from_utf8_lossy(&ok3.stderr)
    );
    assert!(
        anth_seen.lock().unwrap()[0]
            .1
            .iter()
            .any(|h| h == "anthropic-version: 2023-06-01")
    );

    let ok4 = tuse(
        &dir,
        &[
            "connect",
            "--base",
            &gem_url,
            "--key",
            "$TEST_API_KEY",
            "--name",
            "gem",
            "--api",
            "google-gemini",
        ],
    );
    assert!(
        ok4.status.success(),
        "{}",
        String::from_utf8_lossy(&ok4.stderr)
    );
    assert!(
        gem_seen.lock().unwrap()[0]
            .1
            .iter()
            .any(|h| h == "x-goog-api-key: secret-val")
    );

    let plain = tuse(&dir, &["providers"]);
    assert!(plain.status.success());
    let plain_out = String::from_utf8(plain.stdout).unwrap();
    let lines: Vec<&str> = plain_out.lines().collect();
    assert_eq!(lines.len(), 4);
    for line in &lines {
        assert_eq!(
            line.split('\t').count(),
            2,
            "must stay strictly two columns: {line}"
        );
    }
    assert!(!plain_out.contains("secret-val") && !plain_out.contains("TEST_API_KEY"));

    let js = tuse(&dir, &["providers", "--json"]);
    assert!(js.status.success());
    let js_out = String::from_utf8(js.stdout).unwrap();
    assert!(!js_out.contains("secret-val") && !js_out.contains("TEST_API_KEY"));
    assert!(!js_out.contains("api_key"), "{js_out}");
    for expected in [
        r#""api": "openai-chat""#,
        r#""api": "openai-responses""#,
        r#""api": "anthropic-messages""#,
        r#""api": "google-gemini""#,
    ] {
        assert!(js_out.contains(expected), "missing {expected} in {js_out}");
    }

    let _ = std::fs::remove_dir_all(&dir);
}
