//! API route registration.
use crate::{api::middleware::account as account_middleware, runtime::AppState};
use axum::{
    Router,
    extract::DefaultBodyLimit,
    middleware,
    routing::{any, get, post},
};
use std::sync::Arc;
pub mod account;
pub mod frontend;
pub mod health;
pub mod public;
pub mod system;
pub const API_V1_PREFIX: &str = "/api/v1";
pub fn api_router(state: Arc<AppState>) -> Router<Arc<AppState>> {
    let protected = account::router()
        .route(
            "/management/settings",
            get(system::settings).put(system::update_settings),
        )
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            account_middleware::require_ready,
        ));
    let bootstrap = Router::new()
        .route("/setup/status", get(system::status))
        .nest("/public", public::router())
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            account_middleware::bootstrap,
        ));
    Router::new()
        .merge(protected)
        .route("/setup", post(system::initialize))
        .route_layer(middleware::from_fn_with_state(
            state,
            account_middleware::protect,
        ))
        .merge(bootstrap)
        .layer(DefaultBodyLimit::max(16 * 1024))
        .fallback(any(crate::api::common::api_not_found))
}
