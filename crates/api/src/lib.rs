mod args;
mod client;
mod stream;
mod types;

pub use client::{Client, RETRY_DELAYS, Retry, default_agent};
pub use types::*;
pub use ureq::Agent as HttpAgent;
