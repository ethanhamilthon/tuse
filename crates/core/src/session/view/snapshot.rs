use crate::mode::Mode;
use crate::session::Session;
use crate::session::view::history::HistoryItem;
use serde::Serialize;
use std::path::{Path, PathBuf};
use tuse_config::{ModelDef, Prices, split_model};
use tuse_lua::mlua::{self, Lua, LuaSerdeExt, Value};

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
    pub started: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created: Option<String>,
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
    pub history: Option<Vec<HistoryItem>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct UsageSnapshot {
    pub input: u64,
    pub output: u64,
    pub cache_read: u64,
    pub cache_write: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost: Option<f64>,
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
            prices: m.prices.as_ref().map(Into::into),
        }
    }
}

impl SessionSnapshot {
    pub fn pre_session(mode: Mode, configpath: Option<String>, cwd: &Path, home: &Path) -> Self {
        Self {
            mode: mode.name(),
            configpath: configpath.filter(|p| !p.is_empty()),
            cwd: cwd.display().to_string(),
            home: home.display().to_string(),
            id: None,
            started: None,
            created: None,
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
            history: None,
        }
    }

    pub fn to_lua(&self, lua: &Lua) -> mlua::Result<Value> {
        let v = serde_json::to_value(self).map_err(mlua::Error::runtime)?;
        lua.to_value_with(
            &v,
            mlua::serde::SerializeOptions::new().serialize_none_to_null(false),
        )
    }
}

pub(crate) struct LoadCtx {
    pub mode: Mode,
    pub cwd: PathBuf,
    pub configpath: Option<String>,
    pub loading: bool,
}

impl Session {
    pub fn snapshot(&self, include_history: bool) -> SessionSnapshot {
        let config = self.config();
        let cache = self.cached(&mut self.state.lock().unwrap());
        let meta = &cache.meta;
        let (model_full, effort) = (meta.model.clone(), meta.effort.clone());
        let name = meta.title.clone();
        let stats = &cache.stats;
        let def = config.model_def(&model_full);
        let provider = {
            let p = split_model(&model_full).0;
            (!p.is_empty()).then(|| p.to_string())
        };
        let ctx_size = def.filter(|d| d.window > 0).map(|d| d.window);
        let history =
            include_history.then(|| cache.records.iter().map(HistoryItem::from).collect());
        SessionSnapshot {
            mode: self.mode.name(),
            configpath: (!config.path.is_empty()).then(|| config.path.clone()),
            cwd: self.cwd.display().to_string(),
            home: tuse_config::tuse_home().display().to_string(),
            id: Some(meta.id.clone()),
            started: Some(meta.started),
            created: Some(meta.created.clone()),
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
            history,
        }
    }
}
