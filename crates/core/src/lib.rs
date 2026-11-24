mod convo;
mod docs;
mod event;
mod mode;
mod prompt;
mod scripting;
mod session;
mod storage;
mod text;
mod tools;

pub use convo::input::{FILE_LIMIT, Refs, build as build_input};
pub use convo::record::{FileDiff, Record, UserInput};
pub use convo::usage::{Stats, tokens};
pub use docs::docs;
pub use event::{Event, InputId, UiEntry, UiMarkdown, UiPrompt, UiRequest, UiSnapshot};
pub use mode::{Mode, Options, Start};
pub use session::Session;
pub use session::config_lua::{config_arg, configured_models};
pub use session::hooks::EVENTS;
pub use session::models::model_available;
pub use session::open::openable_url;
pub use session::snapshot::{ModelInfoSnapshot, PricesSnapshot, SessionSnapshot, UsageSnapshot};
pub use session::trace::TraceItem;
pub use storage::db::{
    add_provider, default_effort, default_model, providers, providers_json, remove_provider,
    set_state,
};
pub use storage::store::{Meta, list_sessions};
pub use storage::tasks::{TaskInfo, tail};
pub use tuse_config::HOST_FUNCTIONS;
pub use tuse_config::tuse_home;

pub fn supported_efforts(api: &str) -> &'static [&'static str] {
    tuse_api::ApiType::from_stored(api).supported_efforts()
}
