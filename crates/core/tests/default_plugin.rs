use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tuse_core::{Event, Mode, Options, Session, Start, UiPrompt};

static ENV_LOCK: Mutex<()> = Mutex::new(());

fn open(dir: &Path, cfg: &Path) -> Result<Arc<Session>, String> {
    Session::open(Options {
        cwd: dir.to_path_buf(),
        config: Some(cfg.to_path_buf()),
        mode: Mode::Json,
        resume: None,
        start: Start::Launch,
    })
}

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

fn wait_for_ui_request(s: &Session) -> (u64, UiPrompt) {
    let dl = Instant::now() + Duration::from_secs(10);
    while Instant::now() < dl {
        if let Some(req) = s.ui_snapshot().active {
            return (req.id, req.prompt);
        }
        std::thread::sleep(Duration::from_millis(15));
    }
    panic!("timed out waiting for ui_request");
}

fn wait_subs_md<F: Fn(&str) -> bool>(rx: &Receiver<Event>, pred: F) -> String {
    let dl = Instant::now() + Duration::from_secs(10);
    let mut seen = vec![];
    while Instant::now() < dl {
        let rem = dl.saturating_duration_since(Instant::now());
        if let Ok(ev) = rx.recv_timeout(rem.min(Duration::from_millis(100)))
            && let Event::UiMarkdown { name, text, .. } = ev
            && name == "subs"
        {
            if pred(&text) {
                return text;
            }
            seen.push(text);
        }
    }
    panic!("timed out waiting for subs markdown; saw: {seen:?}");
}

fn pick_answer(s: &Arc<Session>, choice: &str) {
    let (id, prompt) = wait_for_ui_request(s);
    match prompt {
        UiPrompt::Picker { options } => {
            assert!(
                options.iter().any(|o| o == choice),
                "choice `{choice}` not in {options:?}"
            );
            s.ui_reply(id, Some(choice.to_string())).unwrap();
        }
        other => panic!("expected picker, got {other:?}"),
    }
}

fn pid_alive(pid: u32) -> bool {
    std::process::Command::new("kill")
        .args(["-0", &pid.to_string()])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .unwrap()
        .success()
}

fn wait_dead(pid: u32) -> bool {
    let dl = Instant::now() + Duration::from_secs(3);
    while Instant::now() < dl {
        if !pid_alive(pid) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    !pid_alive(pid)
}

fn write_mock_proxy_script(path: &Path) {
    let script = r#"#!/usr/bin/env python3
import http.server, json, os, re, sys, time

args = sys.argv[1:]
if "--help" in args or "--version" in args:
    print("CLIProxyAPI Version: 8.1.2, Commit: abcdef1, BuiltAt: 2026-11-23")
    sys.exit(0)

cfg_path = args[args.index("--config") + 1] if "--config" in args else ""
cfg_text = open(cfg_path).read() if cfg_path and os.path.exists(cfg_path) else ""
host = re.search(r'host:\s*"([^"]+)"', cfg_text).group(1) if re.search(r'host:\s*"([^"]+)"', cfg_text) else "127.0.0.1"
port = int(re.search(r'port:\s*(\d+)', cfg_text).group(1)) if re.search(r'port:\s*(\d+)', cfg_text) else 8317
auth_dir = re.search(r'auth-dir:\s*"([^"]+)"', cfg_text).group(1) if re.search(r'auth-dir:\s*"([^"]+)"', cfg_text) else ""
key_m = re.search(r'api-keys:\s*\n\s*-\s*"([^"]+)"', cfg_text)
api_key = key_m.group(1) if key_m else ""

if "--codex-login" in args or "--claude-login" in args or "--antigravity-login" in args:
    if os.environ.get("MOCK_LOGIN_HANG") == "1":
        if os.environ.get("MOCK_LOGIN_PIDFILE"):
            open(os.environ["MOCK_LOGIN_PIDFILE"], "w").write(str(os.getpid()))
        print("Open https://auth.example.test/authorize?state=ok&code=secret_oauth_code_99", flush=True)
        print("Enter code: ABCD-EFGH", flush=True)
        print("access_token=tok_should_not_leak", flush=True)
        time.sleep(60)
        sys.exit(0)
    print("Open https://auth.example.test/authorize?state=ok&code=secret_oauth_code_1", flush=True)
    time.sleep(0.15)
    os.makedirs(auth_dir, exist_ok=True)
    cred = os.path.join(auth_dir, "codex-user.json")
    open(cred, "w").write('{"token":"secret"}')
    os.chmod(cred, 0o644)
    sys.exit(0)

if os.environ.get("MOCK_PROXY_PIDFILE"):
    open(os.environ["MOCK_PROXY_PIDFILE"], "w").write(str(os.getpid()))

print(f"Boot local={api_key} Authorization: Bearer sk-leak-123 access_token=at_leak_456 jwt=eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0NSJ9.sigxyz", flush=True)

class H(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        if self.path == "/v1/models" and self.headers.get("Authorization") == f"Bearer {api_key}":
            body = json.dumps({"data": [{"id": "gpt-5-sub"}]}).encode()
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)
        else:
            self.send_response(401)
            self.end_headers()
    def log_message(self, *a):
        pass

http.server.HTTPServer((host, port), H).serve_forever()
"#;
    std::fs::write(path, script).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

#[test]
fn bundled_default_plugin_theme_validation_and_zero_side_effects() {
    let _guard = ENV_LOCK.lock().unwrap();
    let dir = std::env::temp_dir().join(format!("tuse-default-val-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let home = dir.join("home");
    std::fs::create_dir_all(&home).unwrap();
    unsafe {
        std::env::set_var("HOME", &home);
    }

    let rogue = home.join(".tuse/plugins/tuse/default");
    std::fs::create_dir_all(&rogue).unwrap();
    std::fs::write(
        rogue.join("init.lua"),
        "error('rogue disk checkout executed')",
    )
    .unwrap();

    let cfg = dir.join("tuse.lua");
    let write = |s: &str| std::fs::write(&cfg, s).unwrap();

    write(r#"return { plugins = { "tuse/default" } }"#);
    let s = open(&dir, &cfg).unwrap();
    let c = s.config();
    assert_eq!(c.theme.bg, "#0F1117");
    assert_eq!(c.theme.accent, "#7EE787");
    assert!(c.commands.is_empty());
    assert_eq!(c.plugins[0].commit.as_deref(), Some("bundled"));
    s.close();

    write(r##"return { plugins = { "tuse/default" }, theme = { accent = "#FFB86C" } }"##);
    let s = open(&dir, &cfg).unwrap();
    assert_eq!(
        (
            s.config().theme.bg.as_str(),
            s.config().theme.accent.as_str()
        ),
        ("#0F1117", "#FFB86C")
    );
    s.close();

    let core_default = tuse_config::Theme::default();
    write(r#"return { plugins = { { "tuse/default", opts = { theme = false } } } }"#);
    let s = open(&dir, &cfg).unwrap();
    assert_eq!(s.config().theme.bg, core_default.bg);
    assert_eq!(s.config().theme.accent, core_default.accent);
    s.close();

    let subs_dir = dir.join("subs-noside");
    write(&format!(
        r#"return {{ plugins = {{ {{ "tuse/default", opts = {{ subs = {{ config_dir = {:?} }} }} }} }} }}"#,
        subs_dir.display().to_string()
    ));
    let s = open(&dir, &cfg).unwrap();
    assert_eq!(s.config().theme.bg, "#0F1117");
    assert_eq!(s.config().commands, ["subs"]);
    s.reload().unwrap();
    assert!(!subs_dir.exists());
    assert!(s.tasks.list().is_empty());
    assert!(tuse_core::providers().unwrap().is_empty());
    s.close();

    write(&format!(
        r#"return {{ plugins = {{ {{ "tuse/default", opts = {{ theme = false, subs = {{ config_dir = {:?} }} }} }} }} }}"#,
        subs_dir.display().to_string()
    ));
    let s = open(&dir, &cfg).unwrap();
    assert_eq!(s.config().theme.bg, core_default.bg);
    assert_eq!(s.config().commands, ["subs"]);
    s.close();

    for (lua_src, want) in [
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
            "plugin tuse/default: opts.bogus: unknown key",
        ),
        (
            r#"return { plugins = { { "tuse/default", opts = { subs = { bogus = 1 } } } } }"#,
            "plugin tuse/default: opts.subs.bogus: unknown key",
        ),
        (
            r#"return { plugins = { { "tuse/default", opts = { subs = { host = "0.0.0.0" } } } } }"#,
            "plugin tuse/default: opts.subs.host:",
        ),
        (
            r#"return { plugins = { { "tuse/default", opts = { subs = { host = "::" } } } } }"#,
            "plugin tuse/default: opts.subs.host:",
        ),
        (
            r#"return { plugins = { { "tuse/default", opts = { subs = { host = "192.168.1.5" } } } } }"#,
            "plugin tuse/default: opts.subs.host:",
        ),
        (
            r#"return { plugins = { { "tuse/default", opts = { subs = { port = 0 } } } } }"#,
            "plugin tuse/default: opts.subs.port:",
        ),
        (
            r#"return { plugins = { { "tuse/default", opts = { subs = { port = 70000 } } } } }"#,
            "plugin tuse/default: opts.subs.port:",
        ),
        (
            r#"return { plugins = { { "tuse/default", opts = { subs = { provider = "a/b" } } } } }"#,
            "plugin tuse/default: opts.subs.provider:",
        ),
        (
            r#"return { plugins = { { "tuse/default", opts = { subs = { provider = "a b" } } } } }"#,
            "plugin tuse/default: opts.subs.provider:",
        ),
    ] {
        write(lua_src);
        let err = open(&dir, &cfg).err().unwrap();
        assert!(err.contains(want), "for {lua_src}: got {err}");
    }

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn subs_setup_digest_login_streaming_ownership_reload_and_redaction() {
    let _guard = ENV_LOCK.lock().unwrap();
    let dir = std::env::temp_dir().join(format!("tuse-default-subs-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let home = dir.join("home");
    std::fs::create_dir_all(&home).unwrap();
    unsafe {
        std::env::set_var("HOME", &home);
    }

    let dl_listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let dl_url = format!("http://{}", dl_listener.local_addr().unwrap());
    std::thread::spawn(move || {
        if let Ok((mut stream, _)) = dl_listener.accept() {
            let mut r = BufReader::new(stream.try_clone().unwrap());
            loop {
                let mut l = String::new();
                if r.read_line(&mut l).unwrap_or(0) == 0 || l.trim().is_empty() {
                    break;
                }
            }
            let body = b"not-a-valid-release-archive";
            let _ = write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            let _ = stream.write_all(body);
        }
    });
    unsafe {
        std::env::set_var("TUSE_SUBS_RELEASE_URL", &dl_url);
    }

    let subs_dir = dir.join("subs");
    let cfg = dir.join("tuse.lua");
    let port = free_port();
    let missing_bin = dir.join("no-such-bin");
    std::fs::write(
        &cfg,
        format!(
            r#"return {{ plugins = {{ {{ "tuse/default", opts = {{ theme = false, subs = {{ config_dir = {:?}, binary = {:?}, port = {port} }} }} }} }} }}"#,
            subs_dir.display().to_string(),
            missing_bin.display().to_string()
        ),
    )
    .unwrap();
    let s = open(&dir, &cfg).unwrap();
    let rx = s.subscribe();
    s.run_command("subs").unwrap();
    pick_answer(&s, "setup");
    pick_answer(&s, "Install CLIProxyAPI v8.1.2");
    let md = wait_subs_md(&rx, |t| t.contains("SHA-256 mismatch"));
    assert!(md.contains("opts.subs.binary"), "{md}");
    assert!(
        std::fs::read_dir(&subs_dir)
            .unwrap()
            .flatten()
            .all(|e| !e.file_name().to_string_lossy().starts_with(".stage-"))
    );
    s.close();

    let mock_bin = dir.join("cli-proxy-api");
    write_mock_proxy_script(&mock_bin);
    let write_cfg = |p: u16, enabled: bool| {
        if enabled {
            std::fs::write(
                &cfg,
                format!(
                    r#"return {{
                        commands = {{
                            spawn_decoy = function()
                                local t = require("tuse")
                                t.spawn(function() t.sleep(30) end, "subs-proxy")
                            end
                        }},
                        plugins = {{ {{ "tuse/default", opts = {{ theme = false, subs = {{ config_dir = {:?}, binary = {:?}, port = {p} }} }} }} }}
                    }}"#,
                    subs_dir.display().to_string(),
                    mock_bin.display().to_string()
                ),
            )
            .unwrap();
        } else {
            std::fs::write(
                &cfg,
                r#"return { plugins = { { "tuse/default", opts = { theme = false, subs = false } } } }"#,
            )
            .unwrap();
        }
    };
    write_cfg(port, true);

    let s = open(&dir, &cfg).unwrap();
    let rx = s.subscribe();

    s.run_command("subs").unwrap();
    pick_answer(&s, "setup");
    pick_answer(
        &s,
        &format!("Create local config in {}", subs_dir.display()),
    );
    wait_subs_md(&rx, |t| t.contains("setup complete"));

    let mode = |p: &Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode(&subs_dir), 0o700);
    assert_eq!(mode(&subs_dir.join("auth")), 0o700);
    assert_eq!(mode(&subs_dir.join("api.key")), 0o600);
    assert_eq!(mode(&subs_dir.join("config.yaml")), 0o600);
    let key1 = std::fs::read_to_string(subs_dir.join("api.key")).unwrap();

    s.run_command("subs").unwrap();
    pick_answer(&s, "setup");
    pick_answer(&s, "Keep existing config");
    wait_subs_md(&rx, |t| t.contains("setup complete"));
    assert_eq!(
        std::fs::read_to_string(subs_dir.join("api.key")).unwrap(),
        key1
    );

    s.run_command("subs").unwrap();
    pick_answer(&s, "setup");
    pick_answer(&s, "Reset config (rotates local API key)");
    wait_subs_md(&rx, |t| t.contains("setup complete"));
    let key2 = std::fs::read_to_string(subs_dir.join("api.key")).unwrap();
    assert_ne!(key1, key2);
    let local_key = key2.trim().to_string();

    let login_pidfile = dir.join("login.pid");
    unsafe {
        std::env::set_var("MOCK_LOGIN_HANG", "1");
        std::env::set_var("MOCK_LOGIN_PIDFILE", &login_pidfile);
    }
    s.run_command("subs").unwrap();
    pick_answer(&s, "login");
    pick_answer(&s, "OpenAI / Codex");
    let streaming_md = wait_subs_md(&rx, |t| {
        t.contains("https://auth.example.test/authorize") && t.contains("ABCD-EFGH")
    });
    assert!(streaming_md.contains("ABCD-EFGH"), "{streaming_md}");
    assert!(!streaming_md.contains("secret_oauth_code_99"));
    assert!(!streaming_md.contains("tok_should_not_leak"));
    let login_pid: u32 = std::fs::read_to_string(&login_pidfile)
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    s.cancel();
    assert!(
        wait_dead(login_pid),
        "cancelling login must kill its process group"
    );

    unsafe {
        std::env::remove_var("MOCK_LOGIN_HANG");
    }
    s.run_command("subs").unwrap();
    pick_answer(&s, "login");
    pick_answer(&s, "OpenAI / Codex");
    wait_subs_md(&rx, |t| t.contains("Login complete"));
    assert_eq!(mode(&subs_dir.join("auth/codex-user.json")), 0o600);

    s.run_command("spawn_decoy").unwrap();
    let dl = Instant::now() + Duration::from_secs(5);
    while s.tasks.list().is_empty() && Instant::now() < dl {
        std::thread::sleep(Duration::from_millis(20));
    }
    let decoy_id = s.tasks.list()[0].id.clone();

    s.run_command("subs").unwrap();
    pick_answer(&s, "stop");
    wait_subs_md(&rx, |t| t.contains("No managed proxy"));
    assert_eq!(s.tasks.get(&decoy_id).unwrap().status, "running");

    let proxy_pidfile = dir.join("proxy.pid");
    unsafe {
        std::env::set_var("MOCK_PROXY_PIDFILE", &proxy_pidfile);
    }
    s.run_command("subs").unwrap();
    pick_answer(&s, "start");
    let started_md = wait_subs_md(&rx, |t| t.contains("Proxy started"));
    let owned_id = s
        .tasks
        .list()
        .into_iter()
        .find(|t| t.id != decoy_id && t.name == "subs-proxy")
        .unwrap()
        .id;
    assert!(started_md.contains(&owned_id), "{started_md}");

    let log_text = tuse_core::tail(&s.tasks.get(&owned_id).unwrap().log, 50);
    assert!(log_text.contains("[REDACTED]"), "{log_text}");
    for secret in [&local_key, "sk-leak-123", "at_leak_456", "eyJhbGci"] {
        assert!(
            !log_text.contains(secret),
            "leaked `{secret}` in {log_text}"
        );
    }

    std::fs::write(&cfg, "return { broken_key = 1 }").unwrap();
    assert!(s.reload().is_err());
    assert_eq!(s.tasks.get(&owned_id).unwrap().status, "running");

    write_cfg(port, true);
    s.reload().unwrap();
    assert_eq!(s.tasks.get(&owned_id).unwrap().status, "running");

    write_cfg(port, false);
    s.reload().unwrap();
    let dl = Instant::now() + Duration::from_secs(3);
    while s.tasks.get(&owned_id).unwrap().status == "running" && Instant::now() < dl {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_ne!(s.tasks.get(&owned_id).unwrap().status, "running");
    assert_eq!(s.tasks.get(&decoy_id).unwrap().status, "running");
    let mut saw_notice = false;
    while let Ok(ev) = rx.try_recv() {
        if matches!(ev, Event::Notice(ref msg) if msg.contains("Stopped previous managed subs proxy"))
        {
            saw_notice = true;
        }
    }
    assert!(saw_notice);

    write_cfg(port, true);
    s.reload().unwrap();
    let _ = std::fs::remove_file(&proxy_pidfile);
    s.run_command("subs").unwrap();
    pick_answer(&s, "start");
    wait_subs_md(&rx, |t| t.contains("Proxy started"));
    let pid2: u32 = std::fs::read_to_string(&proxy_pidfile)
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    s.close();
    assert!(
        wait_dead(pid2),
        "session close must kill owned proxy process"
    );

    let mut ext = std::process::Command::new(&mock_bin)
        .args(["--config", subs_dir.join("config.yaml").to_str().unwrap()])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .unwrap();
    let ext_pid = ext.id();
    let dl = Instant::now() + Duration::from_secs(5);
    while Instant::now() < dl {
        if tuse_api::probe_get(
            &format!("http://127.0.0.1:{port}/v1/models"),
            &[("Authorization", &format!("Bearer {local_key}"))],
            Duration::from_millis(200),
        )
        .is_ok_and(|(st, _)| st == 200)
        {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    let s2 = open(&dir, &cfg).unwrap();
    let rx2 = s2.subscribe();
    s2.run_command("subs").unwrap();
    pick_answer(&s2, "start");
    wait_subs_md(&rx2, |t| t.contains("externally managed"));
    s2.run_command("subs").unwrap();
    pick_answer(&s2, "stop");
    wait_subs_md(&rx2, |t| t.contains("externally managed; not stopping"));
    assert!(pid_alive(ext_pid), "stop must never kill an external proxy");
    s2.close();
    let _ = ext.kill();
    let _ = ext.wait();

    let _ = std::fs::remove_dir_all(&dir);
}
