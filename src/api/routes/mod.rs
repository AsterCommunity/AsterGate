//! API route registration.
use axum::{Router, routing::any};
pub mod account;
pub mod frontend;
pub mod health;
pub mod public;
pub mod system;
pub const API_V1_PREFIX: &str = "/api/v1";
pub fn api_router(
    state: std::sync::Arc<crate::runtime::AppState>,
) -> Router<std::sync::Arc<crate::runtime::AppState>> {
    account::router(state).fallback(any(crate::api::common::api_not_found))
}
