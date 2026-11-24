use crate::sh::ShOut;
use mlua::{Lua, LuaSerdeExt, Value};

pub fn to_lua(lua: &Lua, v: &serde_json::Value) -> mlua::Result<Value> {
    let o = mlua::serde::SerializeOptions::new()
        .serialize_none_to_null(false)
        .serialize_unit_to_null(false);
    lua.to_value_with(v, o)
}

pub(crate) fn sh_value(lua: &Lua, o: ShOut) -> Value {
    let t = || -> mlua::Result<Value> {
        let t = lua.create_table()?;
        t.set("ok", o.code == Some(0))?;
        t.set("code", o.code)?;
        t.set("timed_out", o.timed_out)?;
        t.set("out", o.out)?;
        let mt: mlua::Table = lua.named_registry_value("tuse.sh")?;
        t.set_metatable(Some(mt))?;
        Ok(Value::Table(t))
    };
    t().unwrap_or(Value::Nil)
}
