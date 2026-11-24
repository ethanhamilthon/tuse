use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::path::Path;
use std::process::{Command, Output, Stdio};
use std::sync::{Arc, Mutex};

fn tuse(dir: &Path, args: &[&str], sin: Option<&str>) -> Output {
    let mut c = Command::new(env!("CARGO_BIN_EXE_tuse"));
    c.current_dir(dir)
        .env("HOME", dir)
        .env("TEST_API_KEY", "secret-val")
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if sin.is_some() {
        c.stdin(Stdio::piped());
    }
    let mut ch = c.spawn().unwrap();
    if let Some(d) = sin {
        ch.stdin.take().unwrap().write_all(d.as_bytes()).unwrap();
    }
    ch.wait_with_output().unwrap()
}

fn mock(body: &'static str) -> (String, Arc<Mutex<Vec<String>>>) {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", l.local_addr().unwrap());
    let seen = Arc::new(Mutex::new(vec![]));
    let log = seen.clone();
    std::thread::spawn(move || {
        for mut s in l.incoming().flatten() {
            let (mut r, mut hdrs) = (BufReader::new(s.try_clone().unwrap()), String::new());
            loop {
                let mut line = String::new();
                if r.read_line(&mut line).unwrap_or(0) == 0 || line.trim().is_empty() {
                    break;
                }
                hdrs.push_str(&line.to_ascii_lowercase());
            }
            log.lock().unwrap().push(hdrs);
            let _ = write!(
                s,
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
        }
    });
    (url, seen)
}

#[test]
fn connect_apis_providers_format_key_stdin_replace_and_probe() {
    let dir = std::env::temp_dir().join(format!("tuse-cli-connect-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let (oai_u, oai_s) = mock(r#"{"data":[{"id":"gpt-5"}]}"#);
    let (anth_u, anth_s) = mock(r#"{"data":[{"id":"claude-4"}],"has_more":false}"#);
    let (gem_u, gem_s) = mock(
        r#"{"models":[{"name":"models/gemini-2.5-pro","supportedGenerationMethods":["generateContent"]}]}"#,
    );

    let bad = tuse(
        &dir,
        &[
            "connect",
            "--base",
            &oai_u,
            "--key",
            "$TEST_API_KEY",
            "--name",
            "bad",
            "--api",
            "bogus",
        ],
        None,
    );
    let err = String::from_utf8(bad.stderr).unwrap();
    assert!(
        !bad.status.success()
            && oai_s.lock().unwrap().is_empty()
            && err.contains("openai-chat")
            && err.contains("google-gemini")
    );

    for (n, u, a) in [
        ("oai", &oai_u, ""),
        ("oai-r", &oai_u, "openai-responses"),
        ("anth", &anth_u, "anthropic-messages"),
        ("gem", &gem_u, "google-gemini"),
    ] {
        let mut args = vec![
            "connect",
            "--base",
            u,
            "--key",
            "$TEST_API_KEY",
            "--name",
            n,
        ];
        if !a.is_empty() {
            args.extend(["--api", a]);
        }
        assert!(tuse(&dir, &args, None).status.success(), "{n}");
    }
    assert!(anth_s.lock().unwrap()[0].contains("anthropic-version: 2023-06-01"));
    assert!(gem_s.lock().unwrap()[0].contains("x-goog-api-key: secret-val"));

    let plain = String::from_utf8(tuse(&dir, &["providers"], None).stdout).unwrap();
    assert!(
        plain.lines().count() == 4
            && plain.lines().all(|l| l.split('\t').count() == 2)
            && !plain.contains("secret-val")
    );
    let js = String::from_utf8(tuse(&dir, &["providers", "--json"], None).stdout).unwrap();
    assert!(!js.contains("secret-val") && !js.contains("api_key"));
    for a in [
        "openai-chat",
        "openai-responses",
        "anthropic-messages",
        "google-gemini",
    ] {
        assert!(js.contains(&format!(r#""api": "{a}""#)));
    }

    let (u1, s1) = mock(r#"{"data":[{"id":"m1"}]}"#);
    let (u2, s2) = mock(r#"{"data":[{"id":"m2"}]}"#);
    let ok = |a: &[&str], sin: Option<&str>| tuse(&dir, a, sin).status.success();
    assert!(!ok(
        &[
            "connect",
            "--base",
            &u1,
            "--key",
            "k",
            "--key-stdin",
            "--name",
            "sub"
        ],
        None
    ));
    assert!(!ok(
        &["connect", "--base", &u1, "--key-stdin", "--name", "sub"],
        Some("\n")
    ));
    assert!(ok(
        &["connect", "--base", &u1, "--key-stdin", "--name", "sub"],
        Some("stdin-1\n")
    ));
    assert!(s1.lock().unwrap()[0].contains("authorization: bearer stdin-1"));

    assert!(!ok(
        &[
            "connect",
            "--base",
            "http://127.0.0.1:1",
            "--key-stdin",
            "--name",
            "sub",
            "--replace"
        ],
        Some("bad\n")
    ));
    assert!(
        String::from_utf8(tuse(&dir, &["providers"], None).stdout)
            .unwrap()
            .contains(&format!("sub\t{u1}"))
    );
    assert!(ok(
        &[
            "connect",
            "--base",
            &u2,
            "--key-stdin",
            "--name",
            "sub",
            "--replace"
        ],
        Some("stdin-2\n")
    ));
    assert!(s2.lock().unwrap()[0].contains("authorization: bearer stdin-2"));
    assert!(
        String::from_utf8(tuse(&dir, &["providers"], None).stdout)
            .unwrap()
            .contains(&format!("sub\t{u2}"))
    );

    let probe = dir.join("probe.lua");
    std::fs::write(
        &probe,
        r#"return { plugins = { { "tuse/default", opts = { theme = false, subs = false } } } }"#,
    )
    .unwrap();
    assert!(ok(&["models", "-c", probe.to_str().unwrap()], None));
    let _ = std::fs::remove_dir_all(&dir);
}
