//! Axum API route assembly.

use axum::{Router, middleware as axum_middleware};
use std::sync::Arc;
use tower_http::compression::CompressionLayer;

pub mod api_error_code;
pub(crate) mod common;
pub(crate) mod cookies;
pub mod dto;
pub(crate) mod extractors;
pub mod http;
pub mod middleware;
#[cfg(all(debug_assertions, feature = "openapi"))]
pub mod openapi;
pub mod response;
pub mod routes;

pub fn router(state: Arc<crate::runtime::AppState>) -> Router {
    #[cfg(feature = "metrics")]
    let metrics = state.metrics.clone();
    let router = Router::new()
        .nest("/api/v1", routes::api_router(state.clone()))
        .merge(routes::health::router(&state))
        .merge(routes::frontend::router())
        .with_state(state.clone())
        .layer(axum_middleware::from_fn(middleware::rate_limit::enforce))
        .layer(axum_middleware::from_fn_with_state(
            middleware::cors::cors_config(state.clone()),
            middleware::cors::cors_boundary,
        ))
        .layer(axum_middleware::from_fn_with_state(
            state.clone(),
            middleware::rate_limit::admission,
        ))
        .layer(axum_middleware::from_fn(
            middleware::response_headers::apply,
        ))
        .layer(axum_middleware::from_fn(
            aster_forge_middleware::axum::security_headers,
        ))
        .layer(axum_middleware::from_fn(
            aster_forge_middleware::axum::request_id,
        ))
        .layer(CompressionLayer::new());
    #[cfg(feature = "metrics")]
    let router = router
        .layer(axum_middleware::from_fn(
            aster_forge_middleware::axum::metrics,
        ))
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
