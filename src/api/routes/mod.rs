//! API route registration.
use axum::{Router, routing::any};
pub mod frontend;
pub mod health;
pub const API_V1_PREFIX: &str = "/api/v1";
pub fn api_router() -> Router<std::sync::Arc<crate::runtime::AppState>> {
    Router::new().fallback(any(crate::api::common::api_not_found))
}
