use serde_json::{Value, json};

pub const PARSE: i64 = -32700;
pub const INVALID: i64 = -32600;
pub const NO_METHOD: i64 = -32601;
pub const PARAMS: i64 = -32602;
pub const SESSION: i64 = -32000;
pub const NOT_READY: i64 = -32002;

pub struct Fail(pub i64, pub String);

impl Fail {
    pub fn params(m: impl Into<String>) -> Self {
        Fail(PARAMS, m.into())
    }
    pub fn session(m: impl Into<String>) -> Self {
        Fail(SESSION, m.into())
    }
}

pub type Reply = Result<Value, Fail>;

pub struct Request {
    pub id: Option<Value>,
    pub method: String,
    pub params: Value,
}

pub fn parse(line: &str) -> Result<Request, (Value, Fail)> {
    let v: Value = serde_json::from_str(line)
        .map_err(|e| (Value::Null, Fail(PARSE, format!("parse error: {e}"))))?;
    let id = v.get("id").cloned();
    let bad = |m: &str| (id.clone().unwrap_or(Value::Null), Fail(INVALID, m.into()));
    if v["jsonrpc"] != "2.0" {
        return Err(bad("`jsonrpc` must be \"2.0\""));
    }
    let Some(method) = v["method"].as_str() else {
        return Err(bad("`method` must be a string"));
    };
    let params = v.get("params").cloned().unwrap_or(json!({}));
    if !params.is_object() {
        return Err(bad("`params` must be an object"));
    }
    Ok(Request {
        id,
        method: method.to_string(),
        params,
    })
}

pub fn answer(id: Value, reply: Reply) -> Value {
    match reply {
        Ok(result) => json!({"jsonrpc": "2.0", "id": id, "result": result}),
        Err(Fail(code, message)) => {
            json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}})
        }
    }
}

pub fn notify(method: &str, params: Value) -> Value {
    json!({"jsonrpc": "2.0", "method": method, "params": params})
}

pub fn str_param(p: &Value, name: &str) -> Result<String, Fail> {
    p[name]
        .as_str()
        .map(String::from)
        .ok_or_else(|| Fail::params(format!("`{name}` must be a string")))
}

pub fn index_param(p: &Value, name: &str) -> Result<usize, Fail> {
    p[name]
        .as_u64()
        .map(|n| n as usize)
        .ok_or_else(|| Fail::params(format!("`{name}` must be a non-negative integer")))
}

pub fn str_list(p: &Value, name: &str) -> Result<Vec<String>, Fail> {
    match &p[name] {
        Value::Null => Ok(vec![]),
        Value::Array(a) => a
            .iter()
            .map(|v| v.as_str().map(String::from))
            .collect::<Option<_>>()
            .ok_or_else(|| Fail::params(format!("`{name}` must be a list of strings"))),
        _ => Err(Fail::params(format!("`{name}` must be a list of strings"))),
    }
}
