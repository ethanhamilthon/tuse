//! Model metadata JSON-RPC handlers (`model.meta.*`).

use crate::proto::{Fail, Reply, str_param};
use serde_json::{Value, json};
use tuse_config::ModelDef;

pub fn list() -> Reply {
    serde_json::to_value(tuse_core::models()).map_err(|e| Fail::session(e.to_string()))
}

pub fn set(p: &Value) -> Reply {
    let def: ModelDef = serde_json::from_value(p.clone())
        .map_err(|e| Fail::params(format!("invalid model metadata: {e}")))?;
    tuse_core::set_model_def(&def).map_err(Fail::session)?;
    Ok(Value::Null)
}

pub fn remove(p: &Value) -> Reply {
    let name = str_param(p, "name")?;
    let removed = tuse_core::remove_model_def(&name).map_err(Fail::session)?;
    Ok(json!(removed))
}
