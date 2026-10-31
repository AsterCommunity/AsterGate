//! Axum API route assembly.

use axum::{Router, middleware};
use std::sync::Arc;
use tower_http::compression::CompressionLayer;

pub(crate) mod account_security;
pub mod api_error_code;
pub(crate) mod common;
pub mod dto;
pub mod http;
#[cfg(all(debug_assertions, feature = "openapi"))]
pub mod openapi;
pub mod response;
pub mod routes;
pub mod transport_security;

pub fn router(state: Arc<crate::runtime::AppState>) -> Router {
    #[cfg(feature = "metrics")]
    let metrics = state.metrics.clone();
    let router = Router::new()
        .nest("/api/v1", routes::api_router(state.clone()))
        .merge(routes::health::router(&state))
        .merge(routes::frontend::router())
        .with_state(state.clone())
        .layer(middleware::from_fn_with_state(
            transport_security::cors_config(state.clone()),
            transport_security::cors_boundary,
        ))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            transport_security::admission,
        ))
        .layer(middleware::from_fn(account_security::response_headers))
        .layer(middleware::from_fn(
            aster_forge_middleware::axum::security_headers,
        ))
        .layer(middleware::from_fn(
            aster_forge_middleware::axum::request_id,
        ))
        .layer(CompressionLayer::new());
    #[cfg(feature = "metrics")]
    let router = router
        .layer(middleware::from_fn(aster_forge_middleware::axum::metrics))
        .layer(axum::Extension(metrics));
    #[cfg(all(debug_assertions, feature = "openapi"))]
    let router = {
        use utoipa::OpenApi;
        router.merge(
            utoipa_swagger_ui::SwaggerUi::new("/swagger-ui")
                .url("/api-docs/openapi.json", openapi::ApiDoc::openapi()),
        )
    };
    router
}
