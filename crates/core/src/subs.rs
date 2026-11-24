use crate::event::Event;
use crate::subs_proc::{self, ProcHandle, ProcShared};
use serde_json::json;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::sync::{Arc, Weak, mpsc};
use std::time::Duration;
use tuse_lua::mlua::{self, Lua, MultiValue, Table, Value};
use tuse_lua::{ExtWait, add_wait, yield_wait};

const THEME_LUA: &str = include_str!("../../../plugins/default/theme.lua");
const UTIL_LUA: &str = include_str!("../../../plugins/default/subs_util.lua");
const SUBS_LUA: &str = include_str!("../../../plugins/default/subs.lua");
const INIT_LUA: &str = include_str!("../../../plugins/default/init.lua");

#[derive(Clone)]
struct Owner {
    owner_id: String,
    task_id: String,
    target: String,
    proc: Option<Weak<ProcShared>>,
}

#[derive(Default)]
struct SubsState {
    next: Option<String>,
    owner: Option<Owner>,
    active: Vec<Weak<ProcShared>>,
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
            Some(o) if next.as_deref() != Some(&o.target) => st.owner.take(),
            _ => None,
        }
    };
    if let Some(owner) = stop {
        if let Some(p) = owner.proc.and_then(|w| w.upgrade()) {
            p.kill();
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
    let Some(mut st) = lua.app_data_mut::<SubsState>() else {
        return;
    };
    let procs = st
        .owner
        .take()
        .and_then(|o| o.proc)
        .into_iter()
        .chain(std::mem::take(&mut st.active));
    drop(st);
    for p in procs.filter_map(|w| w.upgrade()) {
        p.kill();
    }
}

pub(crate) fn load_default_plugin(lua: &Lua, opts: Option<Table>) -> Result<Table, String> {
    ensure_state(lua);
    let m = |e: mlua::Error| tuse_lua::message(&e);
    let env = crate::config_lua::file_env(lua).map_err(&m)?;
    let store = match lua
        .named_registry_value::<Value>("tuse_subs_store")
        .map_err(&m)?
    {
        Value::Table(t) => t,
        _ => {
            let t = lua.create_table().map_err(&m)?;
            lua.set_named_registry_value("tuse_subs_store", t.clone())
                .map_err(&m)?;
            t
        }
    };
    let load = |src: &str, name: &str| {
        lua.load(src)
            .set_environment(env.clone())
            .set_name(name)
            .into_function()
            .map_err(&m)
    };
    let theme = load(THEME_LUA, "=tuse/default/theme")?
        .call::<Value>(())
        .map_err(&m)?;
    let rt = runtime(lua).map_err(&m)?;
    let util = load(UTIL_LUA, "=tuse/default/subs_util")?
        .call::<Value>((rt.clone(), store.clone()))
        .map_err(&m)?;
    let subs = load(SUBS_LUA, "=tuse/default/subs")?
        .call::<Value>((rt, util, store))
        .map_err(&m)?;
    let init = load(INIT_LUA, "=tuse/default/init")?
        .call::<mlua::Function>((theme, subs))
        .map_err(&m)?;
    let opts = opts.unwrap_or(lua.create_table().map_err(&m)?);
    match init.call::<Value>(opts).map_err(&m)? {
        Value::Table(t) => Ok(t),
        Value::Nil => lua.create_table().map_err(&m),
        _ => Err("plugin returned non-table".into()),
    }
}

fn runtime(lua: &Lua) -> mlua::Result<Table> {
    let rt = lua.create_table()?;
    let os = if std::env::consts::OS == "macos" {
        "darwin"
    } else {
        std::env::consts::OS
    };
    let arch = match std::env::consts::ARCH {
        "aarch64" => "arm64",
        "x86_64" => "amd64",
        x => x,
    };
    rt.set("platform", format!("{os}/{arch}"))?;
    rt.set("tuse_bin", tuse_bin())?;
    rt.set(
        "is_exec",
        lua.create_function(|_, p: String| {
            Ok(std::fs::metadata(&p)
                .is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0))
        })?,
    )?;
    rt.set("state", lua.create_function(state_op)?)?;
    rt.set(
        "spawn",
        lua.create_function(|lua, (argv, opts): (Table, Option<Table>)| {
            ensure_state(lua);
            subs_proc::spawn(
                lua,
                argv,
                opts,
                &mut lua.app_data_mut::<SubsState>().unwrap().active,
            )
        })?,
    )?;
    rt.set(
        "next",
        yield_wait(lua, lua.create_function(subs_proc::next)?)?,
    )?;
    rt.set("http", yield_wait(lua, lua.create_function(http_op)?)?)?;
    Ok(rt)
}

fn state_op(
    lua: &Lua,
    (op, a, b, c): (String, Option<Value>, Option<Value>, Option<Value>),
) -> mlua::Result<MultiValue> {
    ensure_state(lua);
    let s = |v: Option<Value>| match v {
        Some(Value::String(s)) => Ok(s.to_string_lossy()),
        _ => Err(mlua::Error::runtime("expected string")),
    };
    let mut st = lua.app_data_mut::<SubsState>().unwrap();
    let mv = |vs: Vec<Value>| Ok(MultiValue::from_vec(vs));
    match op.as_str() {
        "set_next" => {
            st.next = Some(s(a)?);
            mv(vec![])
        }
        "owner" => match &st.owner {
            Some(o) => mv(vec![
                Value::String(lua.create_string(&o.task_id)?),
                Value::String(lua.create_string(&o.owner_id)?),
            ]),
            None => mv(vec![]),
        },
        "set_owner" => {
            st.owner = Some(Owner {
                owner_id: s(a)?,
                task_id: s(b)?,
                target: s(c)?,
                proc: None,
            });
            mv(vec![])
        }
        "bind_proc" => {
            let id = s(a)?;
            let ok = if let Some(Value::UserData(ud)) = b
                && let Some(o) = st.owner.as_mut()
                && o.owner_id == id
            {
                o.proc = Some(Arc::downgrade(&ud.borrow::<ProcHandle>()?.0));
                true
            } else {
                false
            };
            mv(vec![Value::Boolean(ok)])
        }
        "kill_owner" => match st.owner.take() {
            Some(o) => {
                if let Some(p) = o.proc.and_then(|w| w.upgrade()) {
                    p.kill();
                }
                mv(vec![Value::String(lua.create_string(o.task_id)?)])
            }
            None => mv(vec![]),
        },
        _ => Err(mlua::Error::runtime(format!("unknown state op: {op}"))),
    }
}

fn http_op(
    lua: &Lua,
    (url, arg, secs): (String, Option<Value>, Option<f64>),
) -> mlua::Result<Value> {
    let (tx, rx) = mpsc::channel();
    let dst = match &arg {
        Some(Value::String(s)) => Some(s.to_string_lossy()),
        _ => None,
    };
    let mut hdrs = vec![];
    if let Some(Value::Table(t)) = arg {
        for pair in t.pairs::<String, String>() {
            hdrs.push(pair?);
        }
    }
    let dur = Duration::from_secs_f64(
        secs.unwrap_or(if dst.is_some() { 60.0 } else { 3.0 })
            .max(0.1),
    );
    std::thread::spawn(move || {
        let res = match dst {
            Some(p) => match tuse_api::download_to_file(&url, Path::new(&p), dur) {
                Ok(()) => json!({"ok": true}),
                Err(err) => json!({"ok": false, "err": err}),
            },
            None => {
                let refs: Vec<_> = hdrs.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
                match tuse_api::probe_get(&url, &refs, dur) {
                    Ok((status, body)) => json!({"ok": true, "status": status, "body": body}),
                    Err(err) => json!({"ok": false, "err": err}),
                }
            }
        };
        let _ = tx.send(Ok(res));
    });
    Ok(Value::Integer(add_wait(
        lua,
        ExtWait {
            rx,
            pause_timeout: false,
            on_cancel: None,
        },
    )? as i64))
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
    "tuse".into()
}
