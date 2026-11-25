mod convo;
mod docs;
mod event;
mod id;
mod mode;
mod prompt;
mod scripting;
mod session;
mod storage;
mod text;
mod tools;

pub use convo::input::{FILE_LIMIT, Refs, build as build_input};
pub use convo::record::{FileDiff, Record, UserInput};
pub use convo::usage::Stats;
pub use docs::docs;
pub use event::{Event, InputId, UiEntry, UiMarkdown, UiPrompt, UiRequest, UiSnapshot};
pub use id::elapsed_since;
pub use mode::{Mode, Options, Start};
pub use session::Session;
pub use session::hooks::lanes::EVENTS;
pub use session::lua::config_lua::{config_arg, configured_models};
pub use session::lua::open::openable_url;
pub use session::view::models::model_available;
pub use session::view::snapshot::{
    ModelInfoSnapshot, PricesSnapshot, SessionSnapshot, UsageSnapshot,
};
pub use session::view::trace::TraceItem;
pub use storage::store::Meta;
pub use storage::tasks::{TaskInfo, Tasks};
pub use storage::{Prefs, Providers};
pub use text::tokens;
pub use tuse_config::HOST_FUNCTIONS;
pub use tuse_config::tuse_home;

pub fn supported_efforts(api: &str) -> &'static [&'static str] {
    tuse_api::ApiType::from_stored(api).supported_efforts()
}
