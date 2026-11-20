mod edit;
mod expand;
mod functions;
mod load;
mod theme;
mod themes;
mod types;

pub use edit::{add_connect, set_key, set_setting};
pub use expand::expand;
pub use load::{Loaded, find, global_path, home, load, resolve_path};
pub use theme::{Theme, UiSettings, limit};
pub use types::*;

#[cfg(test)]
mod tests;
