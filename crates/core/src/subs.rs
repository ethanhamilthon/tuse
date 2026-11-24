use crate::event::Event;
use regex::Regex;
use serde_json::json;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex, OnceLock, Weak};
use std::time::{Duration, Instant};
use tuse_lua::mlua::{self, Lua, Table, UserData, UserDataMethods, UserDataRef, Value};
use tuse_lua::{ExtWait, add_wait, yield_wait};

const THEME_LUA: &str = include_str!("../../../plugins/default/theme.lua");
const SUBS_LUA: &str = include_str!("../../../plugins/default/subs.lua");
const INIT_LUA: &str = include_str!("../../../plugins/default/init.lua");

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct SubsTarget {
    host: String,
    port: u16,
    config_dir: String,
    binary: String,
}

#[derive(Clone)]
pub(crate) struct SubsOwner {
    owner_id: String,
    task_id: String,
    target: SubsTarget,
    proc: Option<Weak<ProcShared>>,
}

#[derive(Default)]
pub(crate) struct SubsState {
    next: Option<SubsTarget>,
    owner: Option<SubsOwner>,
    last_error: Option<String>,
    active: Vec<Weak<ProcShared>>,
}

enum ProcEvent {
    Line(String),
    Exit(Option<i32>, bool),
}

struct ProcShared {
    pid: u32,
    killed: AtomicBool,
    rx: Mutex<Receiver<ProcEvent>>,
}

impl ProcShared {
    fn kill(&self) {
        if !self.killed.swap(true, Ordering::SeqCst) {
            tuse_lua::kill_group(self.pid);
        }
    }
}

impl Drop for ProcShared {
    fn drop(&mut self) {
        self.kill();
    }
}

struct ProcHandle(Arc<ProcShared>);

impl UserData for ProcHandle {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("pid", |_, this, ()| Ok(this.0.pid));
        methods.add_method("kill", |_, this, ()| {
            this.0.kill();
            Ok(())
        });
    }
}

fn ensure_state(lua: &Lua) {
    if lua.app_data_ref::<SubsState>().is_none() {
        lua.set_app_data(SubsState::default());
    }
}

pub(crate) fn begin_prepare(lua: &Lua) {
    ensure_state(lua);
    if let Some(mut st) = lua.app_data_mut::<SubsState>() {
        st.next = None;
    }
}

pub(crate) fn reconcile(lua: &Lua) {
    ensure_state(lua);
    let stop = {
        let mut st = lua.app_data_mut::<SubsState>().unwrap();
        let next = st.next.take();
        match &st.owner {
            Some(owner) if next.as_ref() != Some(&owner.target) => st.owner.take(),
            _ => None,
        }
    };
    if let Some(owner) = stop {
        if let Some(proc) = owner.proc.and_then(|w| w.upgrade()) {
            proc.kill();
        }
        if let Ok(s) = crate::host::session(lua) {
            let _ = s.stop_task(&owner.task_id);
            s.emit(Event::Notice(
                "Stopped previous managed subs proxy (subs disabled or settings changed). Run /subs -> start to launch a new one.".into(),
            ));
        }
    }
}

pub(crate) fn stop_all(lua: &Lua) {
    let (active, owner_proc) = {
        let Some(mut st) = lua.app_data_mut::<SubsState>() else {
            return;
        };
        let owner_proc = st.owner.take().and_then(|o| o.proc);
        (std::mem::take(&mut st.active), owner_proc)
    };
    if let Some(proc) = owner_proc.and_then(|w| w.upgrade()) {
        proc.kill();
    }
    for w in active {
        if let Some(proc) = w.upgrade() {
            proc.kill();
        }
    }
}

pub(crate) fn load_default_plugin(lua: &Lua, opts: Option<Table>) -> Result<Table, String> {
    ensure_state(lua);
    let env = crate::config_lua::file_env(lua).map_err(|e| tuse_lua::message(&e))?;
    let theme_val = lua
        .load(THEME_LUA)
        .set_environment(env.clone())
        .set_name("=tuse/default/theme")
        .eval::<Value>()
        .map_err(|e| tuse_lua::message(&e))?;
    let rt = runtime(lua).map_err(|e| tuse_lua::message(&e))?;
    let subs_fn = lua
        .load(SUBS_LUA)
        .set_environment(env.clone())
        .set_name("=tuse/default/subs")
        .into_function()
        .map_err(|e| tuse_lua::message(&e))?;
    let subs_mod = subs_fn
        .call::<Value>(rt)
        .map_err(|e| tuse_lua::message(&e))?;
    let init_fn = lua
        .load(INIT_LUA)
        .set_environment(env)
        .set_name("=tuse/default/init")
        .into_function()
        .map_err(|e| tuse_lua::message(&e))?;
    let plugin_fn = init_fn
        .call::<mlua::Function>((theme_val, subs_mod))
        .map_err(|e| tuse_lua::message(&e))?;
    let opts = match opts {
        Some(o) => o,
        None => lua.create_table().map_err(|e| e.to_string())?,
    };
    let val = plugin_fn
        .call::<Value>(opts)
        .map_err(|e| tuse_lua::message(&e))?;
    match val {
        Value::Table(t) => Ok(t),
        Value::Nil => lua.create_table().map_err(|e| e.to_string()),
        _ => Err("plugin returned non-table".into()),
    }
}

fn runtime(lua: &Lua) -> mlua::Result<Table> {
    let rt = lua.create_table()?;
    rt.set("redact", lua.create_function(lua_redact)?)?;
    rt.set("sha256_file", lua.create_function(lua_sha256_file)?)?;
    rt.set("random_hex", lua.create_function(lua_random_hex)?)?;
    rt.set("ensure_dir", lua.create_function(lua_ensure_dir)?)?;
    rt.set("write_secret", lua.create_function(lua_write_secret)?)?;
    rt.set("chmod", lua.create_function(lua_chmod)?)?;
    rt.set("tighten_dir_files", lua.create_function(lua_tighten_dir)?)?;
    rt.set("list_files", lua.create_function(lua_list_files)?)?;
    rt.set("read_file", lua.create_function(lua_read_file)?)?;
    rt.set(
        "exists",
        lua.create_function(|_, p: String| Ok(Path::new(&p).exists()))?,
    )?;
    rt.set(
        "is_regular_file",
        lua.create_function(|_, p: String| {
            Ok(std::fs::symlink_metadata(&p).is_ok_and(|m| m.file_type().is_file()))
        })?,
    )?;
    rt.set(
        "is_exec",
        lua.create_function(|_, p: String| Ok(is_exec(Path::new(&p))))?,
    )?;
    rt.set(
        "which",
        lua.create_function(|_, name: String| Ok(which(&name)))?,
    )?;
    rt.set(
        "expand_home",
        lua.create_function(|_, p: String| Ok(expand_home(&p)))?,
    )?;
    rt.set(
        "rename",
        lua.create_function(|_, (a, b): (String, String)| {
            std::fs::rename(&a, &b).map_err(|e| mlua::Error::runtime(format!("{b}: {e}")))
        })?,
    )?;
    rt.set(
        "remove_all",
        lua.create_function(|_, p: String| {
            let path = Path::new(&p);
            if path.is_dir() {
                let _ = std::fs::remove_dir_all(path);
            } else {
                let _ = std::fs::remove_file(path);
            }
            Ok(())
        })?,
    )?;
    rt.set("platform", lua.create_function(lua_platform)?)?;
    rt.set("tuse_bin", lua.create_function(|_, ()| Ok(tuse_bin()))?)?;
    rt.set("spawn_proc", lua.create_function(lua_spawn_proc)?)?;
    rt.set(
        "next_proc",
        yield_wait(lua, lua.create_function(lua_next_proc)?)?,
    )?;
    rt.set("exec", yield_wait(lua, lua.create_function(lua_exec)?)?)?;
    rt.set(
        "probe_get",
        yield_wait(lua, lua.create_function(lua_probe_get)?)?,
    )?;
    rt.set(
        "download_file",
        yield_wait(lua, lua.create_function(lua_download_file)?)?,
    )?;
    rt.set("set_next_target", lua.create_function(lua_set_next_target)?)?;
    rt.set("get_owner", lua.create_function(lua_get_owner)?)?;
    rt.set("set_owner", lua.create_function(lua_set_owner)?)?;
    rt.set("set_owner_proc", lua.create_function(lua_set_owner_proc)?)?;
    rt.set("owns", lua.create_function(lua_owns)?)?;
    rt.set("kill_owner", lua.create_function(lua_kill_owner)?)?;
    rt.set("get_last_error", lua.create_function(lua_get_last_error)?)?;
    rt.set("set_last_error", lua.create_function(lua_set_last_error)?)?;
    Ok(rt)
}

fn lua_redact(_: &Lua, (text, extra): (String, Option<Table>)) -> mlua::Result<String> {
    let mut secrets = vec![];
    if let Some(t) = extra {
        for v in t.sequence_values::<String>() {
            let s = v?;
            if !s.is_empty() {
                secrets.push(s);
            }
        }
    }
    Ok(redact(&text, &secrets))
}

pub(crate) fn redact(text: &str, extra: &[String]) -> String {
    static PATS: OnceLock<Vec<(Regex, &'static str)>> = OnceLock::new();
    let pats = PATS.get_or_init(|| {
        vec![
            (
                Regex::new(r"(?i)(authorization\s*:\s*bearer\s+)\S+").unwrap(),
                "${1}[REDACTED]",
            ),
            (
                Regex::new(r"(?i)(\bbearer\s+)[A-Za-z0-9._~+/=-]{8,}").unwrap(),
                "${1}[REDACTED]",
            ),
            (
                Regex::new(r#"(?i)((?:x-api-key|x-goog-api-key|api[-_]key)\s*[:=]\s*["']?)[^\s"',;}{]+"#)
                    .unwrap(),
                "${1}[REDACTED]",
            ),
            (
                Regex::new(
                    r#"(?i)("?(?:access_token|refresh_token|id_token|client_secret|set-cookie|cookie)"?\s*[:=]\s*["']?)[^\s"',;}{]+"#,
                )
                .unwrap(),
                "${1}[REDACTED]",
            ),
            (
                Regex::new(r#"(?i)([?&]code=)[^&\s"']+"#).unwrap(),
                "${1}[REDACTED]",
            ),
            (
                Regex::new(r"\beyJ[A-Za-z0-9_-]{6,}\.[A-Za-z0-9_-]{6,}(?:\.[A-Za-z0-9_-]*)?")
                    .unwrap(),
                "[REDACTED]",
            ),
        ]
    });
    let mut out = text.to_string();
    for s in extra {
        let s = s.trim();
        if !s.is_empty() {
            out = out.replace(s, "[REDACTED]");
        }
    }
    for (re, rep) in pats {
        out = re.replace_all(&out, *rep).into_owned();
    }
    out
}

fn lua_sha256_file(_: &Lua, path: String) -> mlua::Result<String> {
    let bytes = std::fs::read(&path).map_err(|e| mlua::Error::runtime(format!("{path}: {e}")))?;
    Ok(sha256_hex(&bytes))
}

pub(crate) fn sha256_hex(data: &[u8]) -> String {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];
    let bit_len = (data.len() as u64).wrapping_mul(8);
    let mut msg = data.to_vec();
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bit_len.to_be_bytes());
    for chunk in msg.as_chunks::<64>().0 {
        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes(chunk[i * 4..i * 4 + 4].try_into().unwrap());
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh] = h;
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let t1 = hh
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(maj);
            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }
        for (slot, delta) in h.iter_mut().zip([a, b, c, d, e, f, g, hh]) {
            *slot = slot.wrapping_add(delta);
        }
    }
    h.iter().map(|v| format!("{v:08x}")).collect()
}

fn lua_random_hex(_: &Lua, n: usize) -> mlua::Result<String> {
    let mut buf = vec![0u8; n.clamp(1, 128)];
    getrandom::fill(&mut buf).map_err(|e| mlua::Error::runtime(e.to_string()))?;
    Ok(buf.iter().map(|b| format!("{b:02x}")).collect())
}

fn lua_ensure_dir(_: &Lua, (path, mode): (String, u32)) -> mlua::Result<()> {
    std::fs::create_dir_all(&path).map_err(|e| mlua::Error::runtime(format!("{path}: {e}")))?;
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(mode))
        .map_err(|e| mlua::Error::runtime(format!("{path}: {e}")))
}

fn lua_write_secret(_: &Lua, (path, content, mode): (String, String, u32)) -> mlua::Result<()> {
    let dst = Path::new(&path);
    let mut rnd = [0u8; 8];
    let _ = getrandom::fill(&mut rnd);
    let tag: String = rnd.iter().map(|b| format!("{b:02x}")).collect();
    let tmp = dst.with_file_name(format!(
        ".{}.{tag}.tmp",
        dst.file_name().and_then(|n| n.to_str()).unwrap_or("subs")
    ));
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(mode)
        .open(&tmp)
        .map_err(|e| mlua::Error::runtime(format!("{}: {e}", tmp.display())))?;
    let res = (|| -> std::io::Result<()> {
        file.set_permissions(std::fs::Permissions::from_mode(mode))?;
        file.write_all(content.as_bytes())?;
        file.sync_all()?;
        Ok(())
    })();
    drop(file);
    if let Err(e) = res {
        let _ = std::fs::remove_file(&tmp);
        return Err(mlua::Error::runtime(format!("{path}: {e}")));
    }
    std::fs::rename(&tmp, dst).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        mlua::Error::runtime(format!("{path}: {e}"))
    })
}

fn lua_chmod(_: &Lua, (path, mode): (String, u32)) -> mlua::Result<()> {
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(mode))
        .map_err(|e| mlua::Error::runtime(format!("{path}: {e}")))
}

fn lua_tighten_dir(_: &Lua, (dir, mode): (String, u32)) -> mlua::Result<()> {
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Ok(());
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if std::fs::symlink_metadata(&path).is_ok_and(|m| m.file_type().is_file()) {
            let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(mode));
        }
    }
    Ok(())
}

fn lua_list_files(lua: &Lua, dir: String) -> mlua::Result<Table> {
    let mut names = vec![];
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if std::fs::symlink_metadata(&path).is_ok_and(|m| m.file_type().is_file())
                && let Some(name) = path.file_name().and_then(|n| n.to_str())
            {
                names.push(name.to_string());
            }
        }
    }
    names.sort();
    let t = lua.create_table()?;
    for n in names {
        t.push(n)?;
    }
    Ok(t)
}

fn lua_read_file(_: &Lua, path: String) -> mlua::Result<Option<String>> {
    Ok(std::fs::read_to_string(&path).ok())
}

fn is_exec(p: &Path) -> bool {
    std::fs::metadata(p).is_ok_and(|m| m.is_file() && (m.permissions().mode() & 0o111 != 0))
}

fn which(name: &str) -> Option<String> {
    let path_var = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path_var) {
        let cand = dir.join(name);
        if is_exec(&cand) {
            return Some(cand.display().to_string());
        }
    }
    None
}

fn expand_home(p: &str) -> String {
    if p == "~" {
        return std::env::home_dir()
            .unwrap_or_default()
            .display()
            .to_string();
    }
    if let Some(rest) = p.strip_prefix("~/") {
        return std::env::home_dir()
            .unwrap_or_default()
            .join(rest)
            .display()
            .to_string();
    }
    p.to_string()
}

fn lua_platform(lua: &Lua, (): ()) -> mlua::Result<Table> {
    let os = match std::env::consts::OS {
        "macos" => "darwin",
        other => other,
    };
    let arch = match std::env::consts::ARCH {
        "aarch64" => "arm64",
        "x86_64" => "amd64",
        other => other,
    };
    let t = lua.create_table()?;
    t.set("os", os)?;
    t.set("arch", arch)?;
    Ok(t)
}

fn tuse_bin() -> String {
    if let Ok(v) = std::env::var("TUSE_BIN")
        && !v.trim().is_empty()
    {
        return v;
    }
    if let Ok(exe) = std::env::current_exe()
        && exe.file_stem().and_then(|s| s.to_str()) == Some("tuse")
    {
        return exe.display().to_string();
    }
    "tuse".to_string()
}

fn argv_from_table(t: Table) -> mlua::Result<Vec<String>> {
    let mut out = vec![];
    for v in t.sequence_values::<String>() {
        out.push(v?);
    }
    if out.is_empty() {
        return Err(mlua::Error::runtime("argv must not be empty"));
    }
    Ok(out)
}

fn lua_spawn_proc(
    lua: &Lua,
    (argv_t, opts): (Table, Option<Table>),
) -> mlua::Result<mlua::AnyUserData> {
    let argv = argv_from_table(argv_t)?;
    let timeout = match &opts {
        Some(o) => o.get::<Option<f64>>("timeout")?,
        None => None,
    };
    let (reader, writer) = std::io::pipe().map_err(|e| mlua::Error::runtime(e.to_string()))?;
    let mut cmd = Command::new(&argv[0]);
    cmd.args(&argv[1..])
        .stdin(Stdio::null())
        .stdout(
            writer
                .try_clone()
                .map_err(|e| mlua::Error::runtime(e.to_string()))?,
        )
        .stderr(writer);
    unsafe {
        cmd.pre_exec(tuse_lua::setsid);
    }
    let mut child = cmd
        .spawn()
        .map_err(|e| mlua::Error::runtime(format!("{}: {e}", argv[0])))?;
    drop(cmd);
    let pid = child.id();
    let (tx, rx) = mpsc::channel();
    let shared = Arc::new(ProcShared {
        pid,
        killed: AtomicBool::new(false),
        rx: Mutex::new(rx),
    });
    if let Some(mut st) = lua.app_data_mut::<SubsState>() {
        st.active.retain(|w| w.strong_count() > 0);
        st.active.push(Arc::downgrade(&shared));
    }
    let timed_out = Arc::new(AtomicBool::new(false));
    let done = Arc::new(AtomicBool::new(false));
    if let Some(secs) = timeout {
        let s2 = shared.clone();
        let to2 = timed_out.clone();
        let d2 = done.clone();
        let deadline = Instant::now() + Duration::from_secs_f64(secs.max(0.0));
        std::thread::spawn(move || {
            while !d2.load(Ordering::Acquire) && !s2.killed.load(Ordering::Acquire) {
                if Instant::now() >= deadline {
                    to2.store(true, Ordering::Release);
                    s2.kill();
                    break;
                }
                std::thread::sleep(Duration::from_millis(50));
            }
        });
    }
    let d3 = done.clone();
    let to3 = timed_out.clone();
    let s3 = shared.clone();
    std::thread::spawn(move || {
        for line in BufReader::new(reader).lines() {
            let Ok(line) = line else { break };
            if tx.send(ProcEvent::Line(line)).is_err() {
                break;
            }
        }
        let code = child.wait().ok().and_then(|s| s.code());
        s3.killed.store(true, Ordering::SeqCst);
        d3.store(true, Ordering::Release);
        let _ = tx.send(ProcEvent::Exit(code, to3.load(Ordering::Acquire)));
    });
    lua.create_userdata(ProcHandle(shared))
}

fn lua_next_proc(lua: &Lua, handle: UserDataRef<ProcHandle>) -> mlua::Result<Value> {
    let shared = handle.0.clone();
    let cancel_shared = shared.clone();
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let ev = match shared.rx.lock().unwrap().recv() {
            Ok(ProcEvent::Line(line)) => json!({"exited": false, "line": line}),
            Ok(ProcEvent::Exit(code, timed_out)) => {
                json!({"exited": true, "code": code, "timed_out": timed_out})
            }
            Err(_) => json!({"exited": true, "code": null, "timed_out": false}),
        };
        let _ = tx.send(Ok(ev));
    });
    let key = add_wait(
        lua,
        ExtWait {
            rx,
            pause_timeout: false,
            on_cancel: Some(Box::new(move || cancel_shared.kill())),
        },
    )?;
    Ok(Value::Integer(key as i64))
}

fn lua_exec(lua: &Lua, (argv_t, opts): (Table, Option<Table>)) -> mlua::Result<Value> {
    let argv = argv_from_table(argv_t)?;
    let stdin_data = match &opts {
        Some(o) => o.get::<Option<String>>("stdin")?,
        None => None,
    };
    let timeout = match &opts {
        Some(o) => o.get::<Option<f64>>("timeout")?,
        None => None,
    };
    let (mut reader, writer) = std::io::pipe().map_err(|e| mlua::Error::runtime(e.to_string()))?;
    let mut cmd = Command::new(&argv[0]);
    cmd.args(&argv[1..])
        .stdin(if stdin_data.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(
            writer
                .try_clone()
                .map_err(|e| mlua::Error::runtime(e.to_string()))?,
        )
        .stderr(writer);
    unsafe {
        cmd.pre_exec(tuse_lua::setsid);
    }
    let mut child = cmd
        .spawn()
        .map_err(|e| mlua::Error::runtime(format!("{}: {e}", argv[0])))?;
    drop(cmd);
    let pid = child.id();
    if let Some(data) = stdin_data
        && let Some(mut sin) = child.stdin.take()
    {
        std::thread::spawn(move || {
            let _ = sin.write_all(data.as_bytes());
        });
    }
    let cancelled = Arc::new(AtomicBool::new(false));
    let reaped = Arc::new(AtomicBool::new(false));
    let c2 = cancelled.clone();
    let r2 = reaped.clone();
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let out_buf = Arc::new(Mutex::new(Vec::new()));
        let b2 = out_buf.clone();
        let rt_handle = std::thread::spawn(move || {
            let _ = std::io::Read::read_to_end(&mut reader, &mut b2.lock().unwrap());
        });
        let deadline = timeout.map(|s| Instant::now() + Duration::from_secs_f64(s.max(0.0)));
        let mut timed_out = false;
        let code = loop {
            match child.try_wait() {
                Ok(Some(st)) => {
                    r2.store(true, Ordering::SeqCst);
                    break st.code();
                }
                Ok(None) => {
                    if c2.load(Ordering::Acquire) {
                        if !r2.swap(true, Ordering::SeqCst) {
                            tuse_lua::kill_group(pid);
                        }
                        let _ = child.wait();
                        break None;
                    }
                    if deadline.is_some_and(|d| Instant::now() >= d) {
                        timed_out = true;
                        if !r2.swap(true, Ordering::SeqCst) {
                            tuse_lua::kill_group(pid);
                        }
                        let _ = child.wait();
                        break None;
                    }
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(_) => {
                    r2.store(true, Ordering::SeqCst);
                    break None;
                }
            }
        };
        let _ = rt_handle.join();
        let text = String::from_utf8_lossy(&out_buf.lock().unwrap()).into_owned();
        let _ = tx.send(Ok(json!({
            "ok": code == Some(0) && !timed_out,
            "code": code,
            "timed_out": timed_out,
            "out": text,
        })));
    });
    let key = add_wait(
        lua,
        ExtWait {
            rx,
            pause_timeout: false,
            on_cancel: Some(Box::new(move || {
                cancelled.store(true, Ordering::Release);
                if !reaped.swap(true, Ordering::SeqCst) {
                    tuse_lua::kill_group(pid);
                }
            })),
        },
    )?;
    Ok(Value::Integer(key as i64))
}

fn lua_probe_get(
    lua: &Lua,
    (url, headers_t, timeout): (String, Option<Table>, Option<f64>),
) -> mlua::Result<Value> {
    let mut headers = vec![];
    if let Some(t) = headers_t {
        for pair in t.pairs::<String, String>() {
            headers.push(pair?);
        }
    }
    let dur = Duration::from_secs_f64(timeout.unwrap_or(3.0).max(0.1));
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let refs: Vec<(&str, &str)> = headers
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();
        let res = match tuse_api::probe_get(&url, &refs, dur) {
            Ok((status, body)) => json!({"ok": true, "status": status, "body": body}),
            Err(err) => json!({"ok": false, "err": err}),
        };
        let _ = tx.send(Ok(res));
    });
    let key = add_wait(
        lua,
        ExtWait {
            rx,
            pause_timeout: false,
            on_cancel: None,
        },
    )?;
    Ok(Value::Integer(key as i64))
}

fn lua_download_file(
    lua: &Lua,
    (url, dst, timeout): (String, String, Option<f64>),
) -> mlua::Result<Value> {
    let dur = Duration::from_secs_f64(timeout.unwrap_or(60.0).max(1.0));
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let res = match tuse_api::download_to_file(&url, Path::new(&dst), dur) {
            Ok(()) => json!({"ok": true}),
            Err(err) => json!({"ok": false, "err": err}),
        };
        let _ = tx.send(Ok(res));
    });
    let key = add_wait(
        lua,
        ExtWait {
            rx,
            pause_timeout: false,
            on_cancel: None,
        },
    )?;
    Ok(Value::Integer(key as i64))
}

fn target_from_table(t: &Table) -> mlua::Result<SubsTarget> {
    Ok(SubsTarget {
        host: t.get("host")?,
        port: t.get("port")?,
        config_dir: t.get("config_dir")?,
        binary: t.get("binary")?,
    })
}

fn lua_set_next_target(lua: &Lua, t: Table) -> mlua::Result<()> {
    ensure_state(lua);
    let target = target_from_table(&t)?;
    lua.app_data_mut::<SubsState>().unwrap().next = Some(target);
    Ok(())
}

fn lua_get_owner(lua: &Lua, (): ()) -> mlua::Result<Value> {
    ensure_state(lua);
    let owner = lua
        .app_data_ref::<SubsState>()
        .and_then(|s| s.owner.clone());
    let Some(o) = owner else {
        return Ok(Value::Nil);
    };
    let t = lua.create_table()?;
    t.set("owner_id", o.owner_id)?;
    t.set("task_id", o.task_id)?;
    let tgt = lua.create_table()?;
    tgt.set("host", o.target.host)?;
    tgt.set("port", o.target.port)?;
    tgt.set("config_dir", o.target.config_dir)?;
    tgt.set("binary", o.target.binary)?;
    t.set("target", tgt)?;
    Ok(Value::Table(t))
}

fn lua_set_owner(lua: &Lua, v: Value) -> mlua::Result<()> {
    ensure_state(lua);
    let owner = match v {
        Value::Nil => None,
        Value::Table(t) => Some(SubsOwner {
            owner_id: t.get("owner_id")?,
            task_id: t.get("task_id")?,
            target: target_from_table(&t.get::<Table>("target")?)?,
            proc: None,
        }),
        _ => return Err(mlua::Error::runtime("expected table or nil")),
    };
    lua.app_data_mut::<SubsState>().unwrap().owner = owner;
    Ok(())
}

fn lua_set_owner_proc(
    lua: &Lua,
    (owner_id, handle): (String, UserDataRef<ProcHandle>),
) -> mlua::Result<bool> {
    ensure_state(lua);
    let mut st = lua.app_data_mut::<SubsState>().unwrap();
    if let Some(o) = st.owner.as_mut()
        && o.owner_id == owner_id
    {
        o.proc = Some(Arc::downgrade(&handle.0));
        return Ok(true);
    }
    Ok(false)
}

fn lua_owns(lua: &Lua, owner_id: String) -> mlua::Result<bool> {
    ensure_state(lua);
    Ok(lua
        .app_data_ref::<SubsState>()
        .is_some_and(|s| s.owner.as_ref().is_some_and(|o| o.owner_id == owner_id)))
}

fn lua_kill_owner(lua: &Lua, (): ()) -> mlua::Result<Option<String>> {
    ensure_state(lua);
    let owner = lua.app_data_mut::<SubsState>().unwrap().owner.take();
    let Some(o) = owner else {
        return Ok(None);
    };
    if let Some(proc) = o.proc.and_then(|w| w.upgrade()) {
        proc.kill();
    }
    Ok(Some(o.task_id))
}

fn lua_get_last_error(lua: &Lua, (): ()) -> mlua::Result<Option<String>> {
    ensure_state(lua);
    Ok(lua
        .app_data_ref::<SubsState>()
        .and_then(|s| s.last_error.clone()))
}

fn lua_set_last_error(lua: &Lua, err: Option<String>) -> mlua::Result<()> {
    ensure_state(lua);
    lua.app_data_mut::<SubsState>().unwrap().last_error = err;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_matches_standard_vectors() {
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn redactor_masks_keys_headers_oauth_fields_and_jwts() {
        let local_key = "deadbeef0123456789abcdef01234567";
        let sample = format!(
            "key={local_key} Authorization: Bearer sk-topsecret x-api-key: ak_123 x-goog-api-key=gk_456 \
             \"access_token\":\"at_secret\" refresh_token=rt_secret \"id_token\":\"it_secret\" \
             client_secret=cs_secret Cookie: sess=abc https://cb.local/?code=oauth_code_1&state=ok \
             jwt=eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.sig_part_here"
        );
        let out = redact(&sample, &[local_key.to_string()]);
        for forbidden in [
            local_key,
            "sk-topsecret",
            "ak_123",
            "gk_456",
            "at_secret",
            "rt_secret",
            "it_secret",
            "cs_secret",
            "sess=abc",
            "oauth_code_1",
            "eyJhbGci",
            "sig_part_here",
        ] {
            assert!(!out.contains(forbidden), "leaked {forbidden} in {out}");
        }
        assert!(out.contains("[REDACTED]"));
        assert!(out.contains("state=ok"));
    }
}
