mod compaction;
mod config_lua;
mod control;
mod db;
mod docs;
mod event;
mod history;
mod hook_events;
mod hooks;
mod host;
mod id;
mod input;
mod mode;
mod models;
mod open;
mod plugins;
mod prompt;
mod record;
mod require;
mod session;
mod snapshot;
mod store;
mod stream;
mod subs;
mod tasks;
mod tool;
mod tools;
mod trace;
mod turn;
mod ui;
mod usage;
mod validate;

pub use config_lua::{config_arg, configured_models};
pub use control::InputId;
pub use db::{
    add_provider, default_effort, default_model, providers, providers_json, remove_provider,
    set_state,
};
pub use docs::docs;
pub use event::{Event, UiEntry, UiMarkdown, UiPrompt, UiRequest, UiSnapshot};
pub use hooks::EVENTS;
pub use input::{FILE_LIMIT, Refs, build as build_input};
pub use models::model_available;
pub use open::openable_url;
pub use record::{FileDiff, Record, UserInput};
pub use session::{Mode, Options, Session, Start};
pub use snapshot::{ModelInfoSnapshot, PricesSnapshot, SessionSnapshot, UsageSnapshot};
pub use store::{Meta, list_sessions};
pub use tasks::{TaskInfo, tail};
pub use trace::TraceItem;
pub use tuse_config::HOST_FUNCTIONS;
pub use tuse_config::tuse_home;
pub use usage::{Stats, tokens};

pub fn supported_efforts(api: &str) -> &'static [&'static str] {
    tuse_api::ApiType::from_stored(api).supported_efforts()
}
