//! Axum API route assembly.

use axum::{Router, middleware};
use std::sync::Arc;
use tower_http::compression::CompressionLayer;

pub(crate) mod common;
pub mod http;
#[cfg(all(debug_assertions, feature = "openapi"))]
pub mod openapi;
pub mod response;
pub mod routes;

pub fn router(state: Arc<crate::runtime::AppState>) -> Router {
    #[cfg(feature = "metrics")]
    let metrics = state.metrics.clone();
    let router = Router::new()
        .nest("/api/v1", routes::api_router())
        .merge(routes::health::router())
        .merge(routes::frontend::router())
        .with_state(state)
        .layer(middleware::from_fn(
            aster_forge_middleware::axum::security_headers,
        ))
        .layer(middleware::from_fn(
            aster_forge_middleware::axum::request_id,
        ))
        .layer(CompressionLayer::new());
    #[cfg(feature = "metrics")]
    let router = router
        .layer(axum::Extension(metrics))
        .layer(middleware::from_fn(aster_forge_middleware::axum::metrics));
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
