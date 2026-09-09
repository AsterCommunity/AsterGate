//! Health API routes.
use crate::api::response::StatusResponse;
use aster_forge_runtime::{HealthComponentReport, SystemHealthReport};
use axum::{Json, Router, extract::State, http::StatusCode, response::IntoResponse, routing::get};
use std::sync::Arc;

pub fn router() -> Router<Arc<crate::runtime::AppState>> {
    let router = Router::new()
        .route("/health", get(health).head(health))
        .route("/health/ready", get(ready).head(ready));
    aster_forge_observability::axum::configure_prometheus_route(router)
}

#[aster_forge_api_docs_macros::path(get, path = "/health", tag = "health", responses((status = 200, description = "Service health status", body = StatusResponse)))]
pub async fn health() -> impl IntoResponse {
    (
        StatusCode::OK,
        Json(StatusResponse {
            service: env!("CARGO_PKG_NAME"),
            status: "ok",
        }),
    )
}

#[aster_forge_api_docs_macros::path(get, path = "/health/ready", tag = "health", responses((status = 200, description = "Service is ready", body = StatusResponse), (status = 503, description = "Service dependency is not ready", body = StatusResponse)))]
pub async fn ready(State(state): State<Arc<crate::runtime::AppState>>) -> impl IntoResponse {
    let started = std::time::Instant::now();
    let reports = vec![check_database(&state).await, check_cache(&state).await];
    let report = SystemHealthReport::with_duration(reports, started.elapsed());
    crate::metrics::record_health_report(aster_forge_runtime::HealthCheckScope::Readiness, &report);
    let ready = report
        .components
        .iter()
        .all(|component| !component.status.is_issue());
    let response = Json(StatusResponse {
        service: env!("CARGO_PKG_NAME"),
        status: if ready { "ready" } else { "not_ready" },
    });
    (
        if ready {
            StatusCode::OK
        } else {
            StatusCode::SERVICE_UNAVAILABLE
        },
        response,
    )
}

async fn check_database(state: &crate::runtime::AppState) -> HealthComponentReport {
    match aster_forge_db::ping_database(state.db_handles.reader()).await {
        Ok(()) => HealthComponentReport::healthy("database", "database ping succeeded"),
        Err(error) => {
            HealthComponentReport::unhealthy("database", format!("database ping failed: {error}"))
        }
    }
}
async fn check_cache(state: &crate::runtime::AppState) -> HealthComponentReport {
    match state.cache.health_check().await {
        Ok(()) => HealthComponentReport::healthy("cache", "cache health check succeeded"),
        Err(error) => {
            HealthComponentReport::unhealthy("cache", format!("cache health check failed: {error}"))
        }
    }
}
