use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tuse_core::{Event, Mode, Options, Session, Start, UiPrompt};

static ENV_LOCK: Mutex<()> = Mutex::new(());

fn open(dir: &Path, cfg: &Path) -> Result<Arc<Session>, String> {
    Session::open(Options {
        cwd: dir.into(),
        config: Some(cfg.into()),
        mode: Mode::Json,
        resume: None,
        start: Start::Launch,
    })
}

fn wait_until(secs: u64, mut f: impl FnMut() -> bool) -> bool {
    let dl = Instant::now() + Duration::from_secs(secs);
    while Instant::now() < dl {
        if f() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    f()
}

fn subs(s: &Arc<Session>, choices: &[&str]) {
    s.run_command("subs").unwrap();
    for c in choices {
        assert!(
            wait_until(10, || {
                let Some(req) = s.ui_snapshot().active else {
                    return false;
                };
                let UiPrompt::Picker { options } = req.prompt else {
                    panic!("expected picker")
                };
                assert!(options.iter().any(|o| o == c), "`{c}` not in {options:?}");
                s.ui_reply(req.id, Some((*c).into())).unwrap();
                true
            }),
            "timeout for `{c}`"
        );
    }
}

fn wait_md(rx: &Receiver<Event>, pred: impl Fn(&str) -> bool) -> String {
    let dl = Instant::now() + Duration::from_secs(10);
    while Instant::now() < dl {
        if let Ok(Event::UiMarkdown { name, text, .. }) =
            rx.recv_timeout(Duration::from_millis(100))
            && name == "subs"
            && pred(&text)
        {
            return text;
        }
    }
    panic!("timeout waiting for subs markdown");
}

fn alive(pid: u32) -> bool {
    Command::new("kill")
        .args(["-0", &pid.to_string()])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .unwrap()
        .success()
}

fn mode(p: &Path) -> u32 {
    std::fs::metadata(p).unwrap().permissions().mode() & 0o777
}

fn write_mock(path: &Path) {
    let s = r#"#!/usr/bin/env python3
import http.server, json, os, re, sys, time
a = sys.argv[1:]
if "--help" in a or "--version" in a: print("CLIProxyAPI Version: 8.1.2"); sys.exit(0)
t = open(a[a.index("--config") + 1]).read() if "--config" in a else ""
host = (re.search(r'host:\s*"([^"]+)"', t) or [None, "127.0.0.1"])[1]
port = int((re.search(r'port:\s*(\d+)', t) or [None, "8317"])[1])
auth = (re.search(r'auth-dir:\s*"([^"]+)"', t) or [None, ""])[1]
key = (re.search(r'api-keys:\s*\n\s*-\s*"([^"]+)"', t) or [None, ""])[1]
if any(f in a for f in ("--codex-login", "--claude-login", "--antigravity-login")):
    if os.environ.get("MOCK_LOGIN_HANG") == "1":
        open(os.environ["MOCK_LOGIN_PIDFILE"], "w").write(str(os.getpid()))
        print("Open https://auth.example.test/authorize?state=ok&code=secret_oauth_code_99\nEnter code: ABCD-EFGH\naccess_token=tok_no_leak", flush=True)
        time.sleep(60); sys.exit(0)
    os.makedirs(auth, exist_ok=True); p = os.path.join(auth, "codex.json")
    open(p, "w").write('{"t":"s"}'); os.chmod(p, 0o644); sys.exit(0)
if os.environ.get("MOCK_PROXY_PIDFILE"): open(os.environ["MOCK_PROXY_PIDFILE"], "w").write(str(os.getpid()))
print(f"Boot local={key} Authorization: Bearer sk-leak-123 x-api-key: ak_1 x-goog-api-key=gk_2 \"access_token\":\"at_1\" refresh_token=rt_2 \"id_token\":\"it_3\" client_secret=cs_4 Cookie: sess=abc jwt=eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0NSJ9.sigxyz", flush=True)
class H(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        ok = self.path == "/v1/models" and self.headers.get("Authorization") == f"Bearer {key}"
        b = json.dumps({"data": [{"id": "m1"}]}).encode() if ok else b""
        self.send_response(200 if ok else 401)
        if ok: self.send_header("Content-Type", "application/json"); self.send_header("Content-Length", str(len(b)))
        self.end_headers(); self.wfile.write(b)
    def log_message(self, *x): pass
http.server.HTTPServer((host, port), H).serve_forever()
"#;
    std::fs::write(path, s).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

#[test]
fn bundled_default_plugin_theme_validation_and_zero_side_effects() {
    let _g = ENV_LOCK.lock().unwrap();
    let dir = std::env::temp_dir().join(format!("tuse-default-val-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let home = dir.join("home");
    std::fs::create_dir_all(home.join(".tuse/plugins/tuse/default")).unwrap();
    std::fs::write(
        home.join(".tuse/plugins/tuse/default/init.lua"),
        "error('rogue')",
    )
    .unwrap();
    unsafe { std::env::set_var("HOME", &home) };

    let cfg = dir.join("tuse.lua");
    std::fs::write(
        &cfg,
        r##"return { plugins = { "tuse/default" }, theme = { accent = "#FFB86C" } }"##,
    )
    .unwrap();
    let s = open(&dir, &cfg).unwrap();
    assert!(
        s.config().theme.bg == "#0F1117"
            && s.config().theme.accent == "#FFB86C"
            && s.config().plugins[0].commit.as_deref() == Some("bundled")
            && s.config().commands.is_empty()
    );
    s.close();

    let subs_dir = dir.join("subs-noside");
    std::fs::write(&cfg, format!(r#"return {{ plugins = {{ {{ "tuse/default", opts = {{ theme = false, subs = {{ config_dir = {:?} }} }} }} }} }}"#, subs_dir.display().to_string())).unwrap();
    let s = open(&dir, &cfg).unwrap();
    assert!(
        s.config().theme.bg == tuse_config::Theme::default().bg && s.config().commands == ["subs"]
    );
    s.reload().unwrap();
    assert!(
        !subs_dir.exists()
            && s.tasks.list().is_empty()
            && tuse_core::providers().unwrap().is_empty()
    );
    s.close();

    for (lua, want) in [
        (
            r#"return { plugins = { { "tuse/default", ref = "v1" } } }"#,
            "bundled plugin does not accept ref, dir, or path",
        ),
        (
            r#"return { plugins = { { "tuse/default", dir = "." } } }"#,
            "bundled plugin does not accept ref, dir, or path",
        ),
        (
            r#"return { plugins = { { "tuse/default", path = "sub" } } }"#,
            "bundled plugin does not accept ref, dir, or path",
        ),
        (
            r#"return { plugins = { { "tuse/default", opts = { bogus = true } } } }"#,
            "opts.bogus: unknown key",
        ),
        (
            r#"return { plugins = { { "tuse/default", opts = { subs = { bogus = 1 } } } } }"#,
            "opts.subs.bogus: unknown key",
        ),
        (
            r#"return { plugins = { { "tuse/default", opts = { subs = { host = "0.0.0.0" } } } } }"#,
            "opts.subs.host:",
        ),
        (
            r#"return { plugins = { { "tuse/default", opts = { subs = { port = 0 } } } } }"#,
            "opts.subs.port:",
        ),
        (
            r#"return { plugins = { { "tuse/default", opts = { subs = { provider = "a/b" } } } } }"#,
            "opts.subs.provider:",
        ),
    ] {
        std::fs::write(&cfg, lua).unwrap();
        assert!(open(&dir, &cfg).err().unwrap().contains(want), "{lua}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn subs_setup_digest_login_streaming_ownership_reload_and_redaction() {
    let _g = ENV_LOCK.lock().unwrap();
    let dir = std::env::temp_dir().join(format!("tuse-default-subs-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let home = dir.join("home");
    std::fs::create_dir_all(&home).unwrap();
    unsafe { std::env::set_var("HOME", &home) };

    let dl = TcpListener::bind("127.0.0.1:0").unwrap();
    let dl_url = format!("http://{}", dl.local_addr().unwrap());
    std::thread::spawn(move || {
        if let Ok((mut s, _)) = dl.accept() {
            let mut r = BufReader::new(s.try_clone().unwrap());
            while {
                let mut l = String::new();
                r.read_line(&mut l).unwrap_or(0) > 0 && !l.trim().is_empty()
            } {}
            let _ = s.write_all(
                b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\nConnection: close\r\n\r\nbad!",
            );
        }
    });
    unsafe { std::env::set_var("TUSE_SUBS_RELEASE_URL", &dl_url) };

    let (subs_dir, cfg) = (dir.join("subs"), dir.join("tuse.lua"));
    let port = TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let mk_cfg = |bin: &Path, extra: &str| {
        format!(
            r#"return {{ {extra} plugins = {{ {{ "tuse/default", opts = {{ theme = false, subs = {{ config_dir = {:?}, binary = {:?}, port = {port} }} }} }} }} }}"#,
            subs_dir.display().to_string(),
            bin.display().to_string()
        )
    };
    std::fs::write(&cfg, mk_cfg(&dir.join("missing"), "")).unwrap();
    let s = open(&dir, &cfg).unwrap();
    let rx = s.subscribe();
    subs(&s, &["setup", "Install CLIProxyAPI v8.1.2"]);
    assert!(wait_md(&rx, |t| t.contains("SHA-256 mismatch")).contains("opts.subs.binary"));
    assert!(
        std::fs::read_dir(&subs_dir)
            .unwrap()
            .flatten()
            .all(|e| !e.file_name().to_string_lossy().starts_with(".stage-"))
    );
    s.close();

    let mock_bin = dir.join("cli-proxy-api");
    write_mock(&mock_bin);
    let on_cfg = mk_cfg(
        &mock_bin,
        r#"commands = { decoy = function() require("tuse").spawn(function() require("tuse").sleep(30) end, "subs-proxy") end },"#,
    );
    let off_cfg =
        r#"return { plugins = { { "tuse/default", opts = { theme = false, subs = false } } } }"#;
    std::fs::write(&cfg, &on_cfg).unwrap();
    let s = open(&dir, &cfg).unwrap();
    let rx = s.subscribe();

    subs(
        &s,
        &[
            "setup",
            &format!("Create local config in {}", subs_dir.display()),
        ],
    );
    wait_md(&rx, |t| t.contains("setup complete"));
    assert!(
        mode(&subs_dir) == 0o700
            && mode(&subs_dir.join("auth")) == 0o700
            && mode(&subs_dir.join("api.key")) == 0o600
            && mode(&subs_dir.join("config.yaml")) == 0o600
    );
    let k1 = std::fs::read_to_string(subs_dir.join("api.key")).unwrap();
    subs(&s, &["setup", "Keep existing config"]);
    wait_md(&rx, |t| t.contains("setup complete"));
    assert_eq!(
        std::fs::read_to_string(subs_dir.join("api.key")).unwrap(),
        k1
    );
    subs(&s, &["setup", "Reset config (rotates local API key)"]);
    wait_md(&rx, |t| t.contains("setup complete"));
    let key = std::fs::read_to_string(subs_dir.join("api.key"))
        .unwrap()
        .trim()
        .to_string();
    assert_ne!(k1.trim(), key);

    let lpid_f = dir.join("login.pid");
    unsafe {
        std::env::set_var("MOCK_LOGIN_HANG", "1");
        std::env::set_var("MOCK_LOGIN_PIDFILE", &lpid_f);
    }
    subs(&s, &["login", "OpenAI / Codex"]);
    let md = wait_md(&rx, |t| {
        t.contains("https://auth.example.test/authorize") && t.contains("ABCD-EFGH")
    });
    assert!(
        !md.contains("secret_oauth_code_99")
            && !md.contains("tok_no_leak")
            && md.contains("state=ok")
    );
    let lpid: u32 = std::fs::read_to_string(&lpid_f)
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    s.cancel();
    assert!(wait_until(3, || !alive(lpid)));

    unsafe { std::env::remove_var("MOCK_LOGIN_HANG") };
    subs(&s, &["login", "OpenAI / Codex"]);
    wait_md(&rx, |t| t.contains("Login complete"));
    assert_eq!(mode(&subs_dir.join("auth/codex.json")), 0o600);

    s.run_command("decoy").unwrap();
    assert!(wait_until(5, || !s.tasks.list().is_empty()));
    let decoy = s.tasks.list()[0].id.clone();
    subs(&s, &["stop"]);
    wait_md(&rx, |t| t.contains("No managed proxy"));
    assert_eq!(s.tasks.get(&decoy).unwrap().status, "running");

    let ppid_f = dir.join("proxy.pid");
    unsafe { std::env::set_var("MOCK_PROXY_PIDFILE", &ppid_f) };
    subs(&s, &["start"]);
    wait_md(&rx, |t| t.contains("Proxy started"));
    let owned = s
        .tasks
        .list()
        .into_iter()
        .find(|t| t.id != decoy && t.name == "subs-proxy")
        .unwrap()
        .id;
    let log = tuse_core::tail(&s.tasks.get(&owned).unwrap().log, 50);
    assert!(log.contains("[REDACTED]"));
    for sec in [
        key.as_str(),
        "sk-leak-123",
        "ak_1",
        "gk_2",
        "at_1",
        "rt_2",
        "it_3",
        "cs_4",
        "sess=abc",
        "eyJhbGci",
    ] {
        assert!(!log.contains(sec), "leaked `{sec}`");
    }

    std::fs::write(&cfg, "return { broken = 1 }").unwrap();
    assert!(s.reload().is_err() && s.tasks.get(&owned).unwrap().status == "running");
    std::fs::write(&cfg, &on_cfg).unwrap();
    s.reload().unwrap();
    assert_eq!(s.tasks.get(&owned).unwrap().status, "running");
    std::fs::write(&cfg, off_cfg).unwrap();
    s.reload().unwrap();
    assert!(
        wait_until(3, || s.tasks.get(&owned).unwrap().status != "running")
            && s.tasks.get(&decoy).unwrap().status == "running"
    );
    let mut noticed = false;
    while let Ok(ev) = rx.try_recv() {
        noticed |=
            matches!(ev, Event::Notice(ref m) if m.contains("Stopped previous managed subs proxy"));
    }
    assert!(noticed);

    std::fs::write(&cfg, &on_cfg).unwrap();
    s.reload().unwrap();
    let _ = std::fs::remove_file(&ppid_f);
    subs(&s, &["start"]);
    wait_md(&rx, |t| t.contains("Proxy started"));
    let pid2: u32 = std::fs::read_to_string(&ppid_f)
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    s.close();
    assert!(wait_until(3, || !alive(pid2)));

    let mut ext = Command::new(&mock_bin)
        .args(["--config", subs_dir.join("config.yaml").to_str().unwrap()])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let (purl, auth) = (
        format!("http://127.0.0.1:{port}/v1/models"),
        format!("Bearer {key}"),
    );
    assert!(wait_until(5, || tuse_api::probe_get(
        &purl,
        &[("Authorization", &auth)],
        Duration::from_millis(200)
    )
    .is_ok_and(|(st, _)| st == 200)));
    let s2 = open(&dir, &cfg).unwrap();
    let rx2 = s2.subscribe();
    subs(&s2, &["start"]);
    wait_md(&rx2, |t| t.contains("externally managed"));
    subs(&s2, &["stop"]);
    wait_md(&rx2, |t| t.contains("externally managed; not stopping"));
    assert!(alive(ext.id()));
    s2.close();
    let _ = ext.kill();
    let _ = ext.wait();
    let _ = std::fs::remove_dir_all(&dir);
}
