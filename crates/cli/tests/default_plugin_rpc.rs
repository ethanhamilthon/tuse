use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::{Command, Stdio};

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

fn write_mock_proxy(path: &Path) {
    let script = r#"#!/usr/bin/env python3
import http.server, json, os, re, sys
args = sys.argv[1:]
if "--help" in args or "--version" in args:
    print("CLIProxyAPI Version: 8.1.2, Commit: abcdef1, BuiltAt: 2026-11-23")
    sys.exit(0)
cfg_path = args[args.index("--config") + 1] if "--config" in args else ""
cfg_text = open(cfg_path).read() if cfg_path and os.path.exists(cfg_path) else ""
host = re.search(r'host:\s*"([^"]+)"', cfg_text).group(1)
port = int(re.search(r'port:\s*(\d+)', cfg_text).group(1))
api_key = re.search(r'api-keys:\s*\n\s*-\s*"([^"]+)"', cfg_text).group(1)
class H(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        if self.path == "/v1/models" and self.headers.get("Authorization") == f"Bearer {api_key}":
            body = json.dumps({"data": [{"id": "sub-model-1"}]}).encode()
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
fn subs_works_end_to_end_over_tuse_json_rpc() {
    let dir = std::env::temp_dir().join(format!("tuse-default-rpc-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let mock_bin = dir.join("cli-proxy-api");
    write_mock_proxy(&mock_bin);
    let subs_dir = dir.join("subs");
    let port = free_port();
    let cfg = dir.join("tuse.lua");
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
    let mut sin = child.stdin.take().unwrap();
    let mut sout = BufReader::new(child.stdout.take().unwrap());
    let mut next_id = 1u64;

    let mut send = |method: &str, params: &str| {
        let id = next_id;
        next_id += 1;
        writeln!(
            sin,
            r#"{{"jsonrpc":"2.0","id":{id},"method":"{method}","params":{params}}}"#
        )
        .unwrap();
        sin.flush().unwrap();
        id
    };

    let read_until = |sout: &mut BufReader<_>, needle: &str| -> String {
        loop {
            let mut line = String::new();
            let n = sout.read_line(&mut line).unwrap();
            assert!(n > 0, "EOF before seeing {needle}");
            if line.contains(needle) {
                return line;
            }
        }
    };

    send("initialize", r#"{"protocolVersion":1}"#);
    read_until(&mut sout, r#""protocolVersion":1"#);

    let extract_req_id = |line: &str| -> u64 {
        let marker = r#""id":"#;
        let pos = line.rfind(marker).unwrap() + marker.len();
        let rest = &line[pos..];
        let end = rest
            .find(|c: char| !c.is_ascii_digit())
            .unwrap_or(rest.len());
        rest[..end].parse().unwrap()
    };

    send("commands.run", r#"{"name":"subs"}"#);
    let req = read_until(&mut sout, r#""type":"ui_request""#);
    send(
        "ui.reply",
        &format!(r#"{{"id":{},"value":"setup"}}"#, extract_req_id(&req)),
    );
    let req2 = read_until(&mut sout, r#""type":"ui_request""#);
    send(
        "ui.reply",
        &format!(
            r#"{{"id":{},"value":"Create local config in {}"}}"#,
            extract_req_id(&req2),
            subs_dir.display()
        ),
    );
    read_until(&mut sout, "subs setup complete");

    send("commands.run", r#"{"name":"subs"}"#);
    let req = read_until(&mut sout, r#""type":"ui_request""#);
    send(
        "ui.reply",
        &format!(r#"{{"id":{},"value":"start"}}"#, extract_req_id(&req)),
    );
    read_until(&mut sout, "Proxy started");

    let other_listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let other_url = format!("http://{}", other_listener.local_addr().unwrap());
    std::thread::spawn(move || {
        if let Ok((mut s, _)) = other_listener.accept() {
            let mut r = BufReader::new(s.try_clone().unwrap());
            loop {
                let mut l = String::new();
                if r.read_line(&mut l).unwrap_or(0) == 0 || l.trim().is_empty() {
                    break;
                }
            }
            let b = r#"{"data":[{"id":"other"}]}"#;
            let _ = write!(
                s,
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{b}",
                b.len()
            );
        }
    });
    let pre = Command::new(env!("CARGO_BIN_EXE_tuse"))
        .current_dir(&dir)
        .env("HOME", &dir)
        .args([
            "connect", "--base", &other_url, "--key", "other-k", "--name", "subs",
        ])
        .output()
        .unwrap();
    assert!(
        pre.status.success(),
        "{}",
        String::from_utf8_lossy(&pre.stderr)
    );

    send("commands.run", r#"{"name":"subs"}"#);
    let req = read_until(&mut sout, r#""type":"ui_request""#);
    send(
        "ui.reply",
        &format!(r#"{{"id":{},"value":"connect"}}"#, extract_req_id(&req)),
    );
    let req_confirm = read_until(&mut sout, "Replace existing provider 'subs'");
    send(
        "ui.reply",
        &format!(
            r#"{{"id":{},"value":"Replace existing provider 'subs' ({other_url})"}}"#,
            extract_req_id(&req_confirm)
        ),
    );
    read_until(&mut sout, "Connected provider");

    send("commands.run", r#"{"name":"subs"}"#);
    let req = read_until(&mut sout, r#""type":"ui_request""#);
    send(
        "ui.reply",
        &format!(r#"{{"id":{},"value":"status"}}"#, extract_req_id(&req)),
    );
    let st_line = read_until(&mut sout, "connected as `subs`");
    assert!(st_line.contains("reachable, 1 models"), "{st_line}");

    std::fs::write(&cfg, "return { broken_key = 1 }").unwrap();
    send("commands.run", r#"{"name":"subs"}"#);
    let req = read_until(&mut sout, r#""type":"ui_request""#);
    send(
        "ui.reply",
        &format!(r#"{{"id":{},"value":"disconnect"}}"#, extract_req_id(&req)),
    );
    let disc_line = read_until(&mut sout, "config reload failed");
    assert!(disc_line.contains("Removed provider `subs`"), "{disc_line}");

    std::fs::write(&cfg, &cfg_body).unwrap();
    send("commands.run", r#"{"name":"subs"}"#);
    let req = read_until(&mut sout, r#""type":"ui_request""#);
    send(
        "ui.reply",
        &format!(r#"{{"id":{},"value":"stop"}}"#, extract_req_id(&req)),
    );
    read_until(&mut sout, "Stopped managed proxy");

    send("shutdown", "{}");
    let status = child.wait().unwrap();
    assert!(status.success());

    let _ = std::fs::remove_dir_all(&dir);
}
