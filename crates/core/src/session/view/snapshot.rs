use crate::mode::Mode;
use crate::session::Session;
use crate::session::view::history::HistoryItem;
use serde::Serialize;
use std::path::{Path, PathBuf};
use tuse_config::{Prices, split_model};
use tuse_lua::mlua::{self, Lua, LuaSerdeExt, Value};

#[derive(Debug, Clone, Default, Serialize)]
pub struct SessionSnapshot {
    pub mode: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config_path: Option<String>,
    pub cwd: String,
    pub home: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effort: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub busy: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub turn: Option<TurnSnapshot>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_window: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<UsageSnapshot>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_info: Option<ModelInfoSnapshot>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requests: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub llm_requests: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub steps: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub turns: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub calls: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub history: Option<Vec<HistoryItem>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TurnSnapshot {
    pub elapsed_ms: u64,
    pub status: String,
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
    pub available: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub window: Option<u64>,
    pub efforts: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prices: Option<PricesSnapshot>,
}

pub type PricesSnapshot = Prices;

impl SessionSnapshot {
    pub fn pre_session(mode: Mode, config_path: Option<String>, cwd: &Path, home: &Path) -> Self {
        Self {
            mode: mode.name(),
            config_path: config_path.filter(|p| !p.is_empty()),
            cwd: cwd.display().to_string(),
            home: home.display().to_string(),
            ..Default::default()
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
    pub config_path: Option<String>,
    pub loading: bool,
}

impl Session {
    pub fn snapshot(&self, include_history: bool) -> SessionSnapshot {
        let config = self.config();
        let mut st = self.state.lock().unwrap();
        let busy = st.busy;
        let turn = busy.then(|| TurnSnapshot {
            elapsed_ms: st.turn_start.elapsed().as_millis() as u64,
            status: st.status.clone(),
        });
        let live_calls = if busy { st.calls } else { 0 };
        let cache = self.cached(&mut st);
        let meta = &cache.meta;
        let (model_full, effort) = (meta.model.clone(), meta.effort.clone());
        let title = meta.title.clone();
        let stats = &cache.stats;
        let def = config.model_def(&model_full);
        let provider = {
            let p = split_model(&model_full).0;
            (!p.is_empty()).then(|| p.to_string())
        };
        let context_window = def.filter(|d| d.window > 0).map(|d| d.window);
        let model_info = (!model_full.is_empty()).then(|| ModelInfoSnapshot {
            name: model_full.clone(),
            available: self.model_available(&model_full),
            display: def
                .filter(|d| !d.display.is_empty())
                .map(|d| d.display.clone()),
            window: context_window,
            efforts: config.model_efforts(&model_full),
            prices: def.and_then(|d| d.prices.clone()),
        });
        let history =
            include_history.then(|| cache.records.iter().map(HistoryItem::from).collect());
        SessionSnapshot {
            mode: self.mode.name(),
            config_path: (!config.path.is_empty()).then(|| config.path.clone()),
            cwd: self.cwd.display().to_string(),
            home: tuse_config::tuse_home().display().to_string(),
            id: Some(meta.id.clone()),
            started: Some(meta.started),
            created: Some(meta.created.clone()),
            title: (!title.is_empty()).then_some(title),
            model: (!model_full.is_empty()).then_some(model_full),
            provider,
            effort: Some(effort),
            busy: Some(busy),
            turn,
            context: Some(stats.context),
            context_window,
            usage: Some(UsageSnapshot {
                input: stats.totals.input,
                output: stats.totals.output,
                cache_read: stats.totals.cache_read,
                cache_write: stats.totals.cache_write,
                cost: stats.priced.then_some(stats.cost),
                llm_cost: stats.priced.then_some(stats.llm_cost),
            }),
            model_info,
            requests: Some(stats.requests),
            llm_requests: Some(stats.llm_requests),
            steps: Some(stats.steps),
            turns: Some(stats.turns),
            calls: Some(stats.calls + live_calls),
            history,
        }
    }
}
