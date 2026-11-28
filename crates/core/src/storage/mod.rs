mod db;
pub(crate) mod plugins;
pub(crate) mod providers;
pub(crate) mod store;
pub(crate) mod tasks;

pub use db::{Prefs, Providers};
pub use plugins::{PluginAddResult, PluginDetail, PluginInfo, Plugins};
pub use providers::{ConnectInput, ProviderPatch, ProviderResult, ProviderView};

#[cfg(test)]
pub(crate) static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
