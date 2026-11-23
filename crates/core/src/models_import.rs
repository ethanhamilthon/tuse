//! Migration helper: imports legacy `model`, `provider`, `effort`, and `models`
//! from an old `tuse.lua` config into SQLite.

use serde::Deserialize;
use std::path::Path;
use tuse_config::{EFFORTS, ModelDef, full_model_name};
use tuse_lua::mlua::Value;

#[derive(Default)]
pub struct ImportReport {
    pub model: Option<String>,
    pub effort: Option<String>,
    pub models: Vec<String>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Legacy {
    model: String,
    provider: String,
    effort: String,
    models: Vec<ModelDef>,
}

/// Evaluates `path` in a temporary Lua VM and imports its legacy model selection and
/// `models` metadata into SQLite.
pub fn import_legacy_models(path: &Path) -> Result<ImportReport, String> {
    if !path.is_file() {
        return Err(format!("{}: file not found", path.display()));
    }
    let cwd = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."))
        .to_path_buf();
    let host = crate::host::SessionHost {
        slot: crate::host::Slot::default(),
        cwd: cwd.clone(),
    };
    let vm = tuse_lua::Vm::new(std::sync::Arc::new(host), |_| {});
    let p = path.to_path_buf();
    let raw = vm.call(move |lua| {
        crate::host::install(lua, crate::host::Slot::default())
            .map_err(|e| tuse_lua::message(&e))?;
        lua.set_app_data(crate::snapshot::LoadCtx {
            mode: crate::Mode::Json,
            cwd: cwd.clone(),
            configpath: Some(p.display().to_string()),
            loading: true,
        });
        crate::require::install(lua, &cwd).map_err(|e| tuse_lua::message(&e))?;
        let val = crate::config_lua::eval_file(lua, &p)?;
        match val {
            Value::Table(t) => Ok(crate::host::to_json(lua, Value::Table(t))),
            Value::Nil => Ok(serde_json::json!({})),
            _ => Err(format!("{}: config must return a table", p.display())),
        }
    })?;
    let legacy: Legacy =
        serde_json::from_value(raw).map_err(|e| format!("{}: {e}", path.display()))?;
    let providers = crate::db::providers();
    let mut report = ImportReport::default();
    if !legacy.model.is_empty() {
        let full = full_model_name(&legacy.model, &legacy.provider, &providers);
        crate::db::set_state("model", &full)?;
        report.model = Some(full);
    }
    if !legacy.effort.is_empty() {
        if !EFFORTS.contains(&legacy.effort.as_str()) {
            return Err(format!(
                "{}: unknown effort `{}`; use one of {}",
                path.display(),
                legacy.effort,
                EFFORTS.join(", ")
            ));
        }
        crate::db::set_state("effort", &legacy.effort)?;
        report.effort = Some(legacy.effort);
    }
    for m in legacy.models {
        if m.name.trim().is_empty() {
            continue;
        }
        crate::db::set_model_def(&m)?;
        report.models.push(m.name);
    }
    Ok(report)
}
