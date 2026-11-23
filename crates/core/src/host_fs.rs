use crate::host::{action, err, session};
use crate::tools;
use tuse_lua::mlua::{self, Lua, Table, Value};
use tuse_lua::with_run;

/// File and search globals: `read`, `write`, `append`, `delete`, `edit`, `grep`.
pub(crate) fn install(lua: &Lua) -> mlua::Result<()> {
    let g = lua.globals();
    g.set("read", lua.create_function(read)?)?;
    g.set("write", lua.create_function(write)?)?;
    g.set("append", lua.create_function(append)?)?;
    g.set("delete", lua.create_function(delete)?)?;
    g.set("edit", lua.create_function(edit)?)?;
    g.set("grep", lua.create_function(grep)?)?;
    Ok(())
}

fn read(lua: &Lua, (path, o): (String, Option<Table>)) -> mlua::Result<String> {
    let (cwd, max_image_bytes) = match session(lua) {
        Ok(s) => {
            action(&s, "reading", &path);
            (s.cwd.clone(), s.config().settings.max_image_bytes)
        }
        Err(_) => (
            tuse_lua::host(lua).cwd(),
            tuse_config::Settings::default().max_image_bytes,
        ),
    };
    let p = tools::resolve(&path, &cwd);
    let lines = match &o {
        Some(t) => t.get::<Option<bool>>("lines")?.unwrap_or(false),
        None => false,
    };
    let opt = |k: &str| -> mlua::Result<Option<usize>> {
        Ok(match &o {
            Some(t) => t.get::<Option<i64>>(k)?.map(|n| n.max(1) as usize),
            None => None,
        })
    };
    let (offset, limit) = (
        opt("offset")?.unwrap_or(1),
        opt("limit")?.unwrap_or(tools::DEFAULT_LIMIT),
    );
    let r = tools::read(&p, lines, offset, limit, max_image_bytes).map_err(err)?;
    if let Some(url) = r.image {
        with_run(lua, |j| j.images.push(url));
    }
    Ok(r.text)
}

fn write(lua: &Lua, (path, content): (String, String)) -> mlua::Result<String> {
    let s = session(lua)?;
    action(&s, "writing", &path);
    let p = tools::resolve(&path, &s.cwd);
    let (msg, diff) = tools::write(&p, &content).map_err(err)?;
    with_run(lua, |j| j.diffs.push((p.display().to_string(), diff)));
    Ok(msg)
}

fn append(lua: &Lua, (path, text): (String, String)) -> mlua::Result<String> {
    let s = session(lua)?;
    action(&s, "appending", &path);
    let p = tools::resolve(&path, &s.cwd);
    let (msg, diff) = tools::append(&p, &text).map_err(err)?;
    with_run(lua, |j| j.diffs.push((p.display().to_string(), diff)));
    Ok(msg)
}

fn delete(lua: &Lua, path: String) -> mlua::Result<String> {
    let s = session(lua)?;
    action(&s, "deleting", &path);
    let p = tools::resolve(&path, &s.cwd);
    let (msg, diff) = tools::delete(&p).map_err(err)?;
    with_run(lua, |j| j.diffs.push((p.display().to_string(), diff)));
    Ok(msg)
}

fn edit(
    lua: &Lua,
    (path, old, new, o): (String, String, String, Option<Table>),
) -> mlua::Result<String> {
    let s = session(lua)?;
    action(&s, "editing", &path);
    let p = tools::resolve(&path, &s.cwd);
    let all = match o {
        Some(t) => t.get::<Option<bool>>("all")?.unwrap_or(false),
        None => false,
    };
    let (msg, diff) = tools::edit(&p, &old, &new, all).map_err(err)?;
    with_run(lua, |j| j.diffs.push((p.display().to_string(), diff)));
    Ok(msg)
}

fn grep(lua: &Lua, (pattern, o): (String, Option<Table>)) -> mlua::Result<String> {
    let s = session(lua)?;
    action(&s, "searching", &pattern);
    let o = o.unwrap_or(lua.create_table()?);
    let n = |k: &str| -> mlua::Result<Option<usize>> {
        Ok(o.get::<Option<i64>>(k)?.map(|n| n.max(0) as usize))
    };
    let paths = grep_paths(&o)?;
    let opts = tools::GrepOpts {
        paths,
        glob: o.get("glob")?,
        ignore_case: o.get::<Option<bool>>("ignore_case")?.unwrap_or(false),
        literal: o.get::<Option<bool>>("literal")?.unwrap_or(false),
        context: n("context")?.unwrap_or(0),
        limit: n("limit")?,
    };
    let stopped = || tuse_lua::stopped(lua).is_some();
    tools::grep(&pattern, opts, &s.cwd, &stopped).map_err(err)
}

fn grep_paths(o: &Table) -> mlua::Result<Vec<String>> {
    let raw = match o.get::<Value>("path")? {
        Value::Nil => o.get::<Value>("paths")?,
        v => v,
    };
    match raw {
        Value::Nil => Ok(vec![]),
        Value::String(s) => Ok(vec![s.to_str()?.to_string()]),
        Value::Table(t) => {
            let mut out = vec![];
            for v in t.sequence_values::<Value>() {
                match v? {
                    Value::String(s) => out.push(s.to_str()?.to_string()),
                    _ => return Err(err("grep `path` list must contain strings".into())),
                }
            }
            Ok(out)
        }
        _ => Err(err("grep `path` must be a string or list of strings".into())),
    }
}
