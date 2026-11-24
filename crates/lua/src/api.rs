use crate::Host;
use crate::job::{Done, Job, Out, Stop};
use crate::sched::{add_proc, is_lua_hook, start, stopped, take_proc, with_run};
use crate::sh::Proc;
use crate::value::{sh_value, to_lua};
use mlua::{Function, HookTriggers, Lua, LuaSerdeExt, MultiValue, Table, Value, VmState};
use std::sync::Arc;
use std::time::{Duration, Instant};

const PRELUDE: &str = include_str!("prelude.lua");

pub fn host(lua: &Lua) -> Arc<dyn Host> {
    lua.app_data_ref::<Arc<dyn Host>>().unwrap().clone()
}

pub fn install(lua: &Lua) -> mlua::Result<()> {
    let g = lua.globals();
    let tuse = lua.create_table()?;
    let marker = lua.create_table()?;
    lua.set_named_registry_value("tuse.sched", marker.clone())?;
    tuse.set("sh_start", lua.create_function(sh_start)?)?;
    tuse.set("block", lua.create_function(block)?)?;
    g.set("print", lua.create_function(print)?)?;
    let io: Table = g.get("io")?;
    io.set("write", lua.create_function(write)?)?;
    g.set("spawn", lua.create_function(spawn)?)?;
    g.set("json", json(lua)?)?;
    lua.load(PRELUDE)
        .set_name("=prelude")
        .call::<()>((tuse.clone(), marker))?;
    lua.set_named_registry_value("tuse.render", tuse.get::<Function>("render")?)?;
    lua.set_named_registry_value("tuse.wrap", tuse.get::<Function>("wrap")?)?;
    lua.set_named_registry_value("tuse.yield_wait", tuse.get::<Function>("yield_wait")?)?;
    wrap(lua, &g, &["spawn"])?;
    let j: Table = g.get("json")?;
    wrap(lua, &j, &["encode", "decode"])?;
    lua.set_named_registry_value("tuse.sh", tuse.get::<Table>("sh_mt")?)?;
    lua.set_global_hook(
        HookTriggers::new().every_nth_instruction(1000),
        |lua, _| match stopped(lua) {
            Some(why) => Err(mlua::Error::runtime(why)),
            None => Ok(VmState::Continue),
        },
    )?;
    Ok(())
}

pub fn wrap(lua: &Lua, t: &Table, names: &[&str]) -> mlua::Result<()> {
    let w: Function = lua.named_registry_value("tuse.wrap")?;
    for n in names {
        let f: Function = t.get(*n)?;
        t.set(*n, w.call::<Function>(f)?)?;
    }
    Ok(())
}

pub fn yield_wait(lua: &Lua, start: Function) -> mlua::Result<Function> {
    let w: Function = lua.named_registry_value("tuse.yield_wait")?;
    w.call(start)
}

fn text_of(lua: &Lua, args: MultiValue, sep: &str) -> mlua::Result<String> {
    let tostring: Function = lua.globals().get("tostring")?;
    let mut parts = vec![];
    for v in args {
        parts.push(match v {
            Value::String(s) => s.to_string_lossy(),
            v => tostring.call::<String>(v)?,
        });
    }
    Ok(parts.join(sep))
}

fn emit(lua: &Lua, s: &str) {
    if with_run(lua, |j| j.print(s)).is_none() {
        eprint!("{s}");
    }
}

fn print(lua: &Lua, args: MultiValue) -> mlua::Result<()> {
    let mut s = text_of(lua, args, "\t")?;
    s.push('\n');
    emit(lua, &s);
    Ok(())
}

fn write(lua: &Lua, args: MultiValue) -> mlua::Result<()> {
    let s = text_of(lua, args, "")?;
    emit(lua, &s);
    Ok(())
}

fn sh_start(lua: &Lua, (cmd, opts): (String, Option<Table>)) -> mlua::Result<u64> {
    let h = host(lua);
    h.action(&cmd);
    let mut cwd = h.cwd();
    let mut env = Vec::new();
    let mut deadline = with_run(lua, |j| j.stop().deadline()).flatten();
    if let Some(o) = opts {
        if let Some(t) = o.get::<Option<f64>>("timeout")? {
            let own = Instant::now() + Duration::from_secs_f64(t.max(0.0));
            deadline = Some(deadline.map_or(own, |d| d.min(own)));
        }
        if let Some(c) = o.get::<Option<String>>("cwd")? {
            cwd = cwd.join(c);
        }
        if let Some(e) = o.get::<Option<Table>>("env")? {
            for pair in e.pairs::<String, String>() {
                env.push(pair?);
            }
        }
    }
    let p = Proc::start(&cmd, &cwd, &env, deadline).map_err(mlua::Error::runtime)?;
    Ok(add_proc(lua, p))
}

fn block(lua: &Lua, (kind, arg): (String, Value)) -> mlua::Result<(bool, Value)> {
    let stop = with_run(lua, |j| j.stop()).unwrap_or_default();
    match kind.as_str() {
        "sleep" => {
            let until =
                Instant::now() + Duration::from_secs_f64(arg.as_f64().unwrap_or(0.0).max(0.0));
            while Instant::now() < until {
                if let Some(why) = stop.reason() {
                    return Ok((false, Value::String(lua.create_string(why)?)));
                }
                std::thread::sleep(Duration::from_millis(10).min(until - Instant::now()));
            }
            Ok((true, Value::Nil))
        }
        _ => {
            let key = arg.as_integer().unwrap_or(0) as u64;
            let p = take_proc(lua, key);
            let p = p.ok_or_else(|| mlua::Error::runtime("no such process"))?;
            let out = p.wait(|| stop.reason().is_some());
            Ok((true, sh_value(lua, out)))
        }
    }
}

fn spawn(lua: &Lua, (f, name): (Function, Option<String>)) -> mlua::Result<String> {
    let h = host(lua);
    let name = name.unwrap_or_default();
    let t = h
        .task_start(&name, is_lua_hook(lua))
        .map_err(mlua::Error::runtime)?;
    let file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&t.log)
        .map_err(|e| mlua::Error::runtime(format!("{}: {e}", t.log.display())))?;
    let id = t.id.clone();
    let job = Job::new(
        lua,
        f,
        MultiValue::new(),
        Stop::new(t.cancel, None),
        Out::File(file),
        move |_, d: Done| h.task_end(&t.id, d),
    )?;
    start(lua, job);
    Ok(id)
}

fn json(lua: &Lua) -> mlua::Result<Table> {
    let t = lua.create_table()?;
    t.set(
        "encode",
        lua.create_function(|lua, v: Value| {
            let j: serde_json::Value = lua.from_value(v)?;
            Ok(j.to_string())
        })?,
    )?;
    t.set(
        "decode",
        lua.create_function(|lua, s: String| {
            let j: serde_json::Value = serde_json::from_str(&s).map_err(mlua::Error::runtime)?;
            to_lua(lua, &j)
        })?,
    )?;
    Ok(t)
}
