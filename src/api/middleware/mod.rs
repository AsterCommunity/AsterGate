//! Gate HTTP adapters; shared protocol/security mechanics remain Forge-owned.
pub(crate) mod account;
pub(crate) mod cors;
pub(crate) mod rate_limit;
pub(crate) mod response_headers;
mod state;
pub use state::HttpSecurity;
