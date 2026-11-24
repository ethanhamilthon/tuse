use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::os::unix::fs::PermissionsExt;
use std::process::{Command, Stdio};

#[test]
fn subs_works_end_to_end_over_tuse_json_rpc() {
    let dir = std::env::temp_dir().join(format!("tuse-default-rpc-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let mock_bin = dir.join("cli-proxy-api");
    let script = r#"#!/usr/bin/env python3
import http.server, json, re, sys
a = sys.argv[1:]
if "--help" in a or "--version" in a: print("CLIProxyAPI Version: 8.1.2"); sys.exit(0)
t = open(a[a.index("--config") + 1]).read()
h, p, k = re.search(r'host:\s*"([^"]+)"', t)[1], int(re.search(r'port:\s*(\d+)', t)[1]), re.search(r'api-keys:\s*\n\s*-\s*"([^"]+)"', t)[1]
class H(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        ok = self.path == "/v1/models" and self.headers.get("Authorization") == f"Bearer {k}"
        b = json.dumps({"data": [{"id": "m1"}]}).encode() if ok else b""
        self.send_response(200 if ok else 401); self.send_header("Content-Length", str(len(b))); self.end_headers(); self.wfile.write(b)
    def log_message(self, *x): pass
http.server.HTTPServer((h, p), H).serve_forever()
"#;
    std::fs::write(&mock_bin, script).unwrap();
    std::fs::set_permissions(&mock_bin, std::fs::Permissions::from_mode(0o755)).unwrap();

    let (subs_dir, cfg) = (dir.join("subs"), dir.join("tuse.lua"));
    let port = TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let cfg_body = format!(
        r#"return {{ plugins = {{ {{ "tuse/default", opts = {{ theme = false, subs = {{ config_dir = {:?}, binary = {:?}, port = {port} }} }} }} }} }}"#,
        subs_dir.display().to_string(),
        mock_bin.display().to_string()
    );
    std::fs::write(&cfg, &cfg_body).unwrap();

    let mut child = Command::new(env!("CARGO_BIN_EXE_tuse"))
        .current_dir(&dir)
        .env("HOME", &dir)
        .args(["json", "-c", cfg.to_str().unwrap()])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let (mut sin, mut sout, mut seq) = (
        child.stdin.take().unwrap(),
        BufReader::new(child.stdout.take().unwrap()),
        1u64,
    );
    let send = |sin: &mut std::process::ChildStdin, seq: &mut u64, m: &str, p: &str| {
        writeln!(
            sin,
            r#"{{"jsonrpc":"2.0","id":{},"method":"{m}","params":{p}}}"#,
            *seq
        )
        .unwrap();
        sin.flush().unwrap();
        *seq += 1;
    };
    let until = |sout: &mut BufReader<_>, needle: &str| loop {
        let mut l = String::new();
        assert!(sout.read_line(&mut l).unwrap() > 0, "EOF before {needle}");
        if l.contains(needle) {
            return l;
        }
    };
    let reply = |sin: &mut _, seq: &mut u64, line: &str, val: &str| {
        let rest = &line[line.rfind(r#""id":"#).unwrap() + 5..];
        let id = &rest[..rest
            .find(|c: char| !c.is_ascii_digit())
            .unwrap_or(rest.len())];
        send(
            sin,
            seq,
            "ui.reply",
            &format!(r#"{{"id":{id},"value":"{val}"}}"#),
        );
    };
    let act = |sin: &mut _, sout: &mut _, seq: &mut u64, a: &str| {
        send(sin, seq, "commands.run", r#"{"name":"subs"}"#);
        let req = until(sout, r#""type":"ui_request""#);
        reply(sin, seq, &req, a);
    };

    send(&mut sin, &mut seq, "initialize", r#"{"protocolVersion":1}"#);
    until(&mut sout, r#""protocolVersion":1"#);

    act(&mut sin, &mut sout, &mut seq, "setup");
    let req = until(&mut sout, r#""type":"ui_request""#);
    reply(
        &mut sin,
        &mut seq,
        &req,
        &format!("Create local config in {}", subs_dir.display()),
    );
    until(&mut sout, "subs setup complete");

    act(&mut sin, &mut sout, &mut seq, "start");
    until(&mut sout, "Proxy started");

    let other = TcpListener::bind("127.0.0.1:0").unwrap();
    let other_url = format!("http://{}", other.local_addr().unwrap());
    std::thread::spawn(move || {
        if let Ok((mut s, _)) = other.accept() {
            let mut r = BufReader::new(s.try_clone().unwrap());
            while {
                let mut l = String::new();
                r.read_line(&mut l).unwrap_or(0) > 0 && !l.trim().is_empty()
            } {}
            let _ = s.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 22\r\nConnection: close\r\n\r\n{\"data\":[{\"id\":\"o1\"}]}");
        }
    });
    assert!(
        Command::new(env!("CARGO_BIN_EXE_tuse"))
            .current_dir(&dir)
            .env("HOME", &dir)
            .args([
                "connect", "--base", &other_url, "--key", "k", "--name", "subs"
            ])
            .status()
            .unwrap()
            .success()
    );

    act(&mut sin, &mut sout, &mut seq, "connect");
    let req = until(&mut sout, "Replace existing provider 'subs'");
    reply(
        &mut sin,
        &mut seq,
        &req,
        &format!("Replace existing provider 'subs' ({other_url})"),
    );
    until(&mut sout, "Connected provider");

    act(&mut sin, &mut sout, &mut seq, "status");
    assert!(until(&mut sout, "connected as `subs`").contains("reachable, 1 models"));

    std::fs::write(&cfg, "return { broken = 1 }").unwrap();
    act(&mut sin, &mut sout, &mut seq, "disconnect");
    assert!(until(&mut sout, "config reload failed").contains("Removed provider `subs`"));

    std::fs::write(&cfg, &cfg_body).unwrap();
    act(&mut sin, &mut sout, &mut seq, "stop");
    until(&mut sout, "Stopped managed proxy");

    send(&mut sin, &mut seq, "shutdown", "{}");
    assert!(child.wait().unwrap().success());
    let _ = std::fs::remove_dir_all(&dir);
}
