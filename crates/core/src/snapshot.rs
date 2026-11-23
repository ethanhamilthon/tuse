//! Single runtime session snapshot shared by `require("tuse").session()`,
//! the TUI `/session` view, and JSON-RPC.

use crate::mode::Mode;
use crate::session::Session;
use crate::trace::TraceItem;
use serde::Serialize;
use std::path::{Path, PathBuf};
use tuse_config::{ModelDef, Prices, split_model};
use tuse_lua::mlua::{self, Lua, Value};

#[derive(Debug, Clone, Serialize)]
pub struct SessionSnapshot {
    pub mode: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub configpath: Option<String>,
    pub cwd: String,
    pub home: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effort: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ctx: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ctx_size: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<UsageSnapshot>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_info: Option<ModelInfoSnapshot>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub steps: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub turns: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace: Option<Vec<TraceItem>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct UsageSnapshot {
    pub input: u64,
    pub output: u64,
    pub cache_read: u64,
    pub cache_write: u64,
    /// `main + llm`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost: Option<f64>,
    /// The part of `cost` that `llm()` requests spent.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub llm_cost: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModelInfoSnapshot {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub window: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compact_at: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prices: Option<PricesSnapshot>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PricesSnapshot {
    pub input: f64,
    pub output: f64,
    pub cache_read: f64,
    pub cache_write: f64,
}

impl From<&Prices> for PricesSnapshot {
    fn from(p: &Prices) -> Self {
        Self {
            input: p.input,
            output: p.output,
            cache_read: p.cache_read,
            cache_write: p.cache_write,
        }
    }
}

impl From<&ModelDef> for ModelInfoSnapshot {
    fn from(m: &ModelDef) -> Self {
        Self {
            name: m.name.clone(),
            display: (!m.display.is_empty()).then(|| m.display.clone()),
            window: (m.window > 0).then_some(m.window),
            compact_at: m.compact_at,
            prices: m.prices.as_ref().map(Into::into),
        }
    }
}

impl SessionSnapshot {
    /// Snapshot returned before a session exists (initial config load and `/reload`).
    pub fn pre_session(mode: Mode, configpath: Option<String>, cwd: &Path, home: &Path) -> Self {
        Self {
            mode: mode.name(),
            configpath: configpath.filter(|p| !p.is_empty()),
            cwd: cwd.display().to_string(),
            home: home.display().to_string(),
            id: None,
            name: None,
            model: None,
            provider: None,
            effort: None,
            ctx: None,
            ctx_size: None,
            usage: None,
            model_info: None,
            steps: None,
            turns: None,
            trace: None,
        }
    }

    pub fn to_lua(&self, lua: &Lua) -> mlua::Result<Value> {
        let v = serde_json::to_value(self).map_err(mlua::Error::runtime)?;
        tuse_lua::to_lua(lua, &v)
    }
}

/// Stored in Lua app_data so `tuse.session()` has consistent pre-session semantics
/// during both initial config load and `/reload`.
pub(crate) struct LoadCtx {
    pub mode: Mode,
    pub cwd: PathBuf,
    pub configpath: Option<String>,
    pub loading: bool,
}

impl Session {
    /// Builds a fresh runtime session snapshot (no credentials).
    pub fn snapshot(&self, include_trace: bool) -> SessionSnapshot {
        let config = self.config();
        let (model_full, effort) = self.model();
        let name = self.title();
        let stats = self.stats();
        let def = config.model_def(&model_full);
        let provider = {
            let p = split_model(&model_full).0;
            (!p.is_empty()).then(|| p.to_string())
        };
        let ctx_size = def.filter(|d| d.window > 0).map(|d| d.window);
        let trace = include_trace.then(|| self.trace());
        SessionSnapshot {
            mode: self.mode.name(),
            configpath: (!config.path.is_empty()).then(|| config.path.clone()),
            cwd: self.cwd.display().to_string(),
            home: crate::store::tuse_home().display().to_string(),
            id: Some(self.id.clone()),
            name: (!name.is_empty()).then_some(name),
            model: (!model_full.is_empty()).then_some(model_full),
            provider,
            effort: Some(effort),
            ctx: Some(stats.context),
            ctx_size,
            usage: Some(UsageSnapshot {
                input: stats.totals.input,
                output: stats.totals.output,
                cache_read: stats.totals.cached,
                cache_write: stats.totals.cache_write,
                cost: stats.priced.then_some(stats.cost),
                llm_cost: stats.priced.then_some(stats.llm_cost),
            }),
            model_info: def.map(Into::into),
            steps: Some(stats.steps),
            turns: Some(stats.turns),
            trace,
        }
    }
}

pub(crate) fn lua_session(lua: &Lua) -> mlua::Result<Value> {
    let pre = lua.app_data_ref::<LoadCtx>().and_then(|ctx| {
        if ctx.loading {
            Some(SessionSnapshot::pre_session(
                ctx.mode,
                ctx.configpath.clone(),
                &ctx.cwd,
                &crate::store::tuse_home(),
            ))
        } else {
            None
        }
    });
    if let Some(snap) = pre {
        return snap.to_lua(lua);
    }
    if let Ok(s) = crate::host::session(lua) {
        return s.snapshot(true).to_lua(lua);
    }
    let snap = lua
        .app_data_ref::<LoadCtx>()
        .map(|ctx| {
            SessionSnapshot::pre_session(
                ctx.mode,
                ctx.configpath.clone(),
                &ctx.cwd,
                &crate::store::tuse_home(),
            )
        })
        .ok_or_else(|| mlua::Error::runtime("session closed"))?;
    snap.to_lua(lua)
}
