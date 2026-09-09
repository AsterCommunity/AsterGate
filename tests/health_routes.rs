//! HTTP health route integration tests.
#[macro_use]
mod common;
use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use tower::ServiceExt;

async fn request(app: &axum::Router, method: &str, uri: &str) -> axum::response::Response {
    app.clone()
        .oneshot(
            Request::builder()
                .method(method)
                .uri(uri)
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should respond")
}
async fn json(response: axum::response::Response) -> serde_json::Value {
    serde_json::from_slice(
        &to_bytes(response.into_body(), 2 * 1024 * 1024)
            .await
            .expect("body should read"),
    )
    .expect("body should be json")
}

#[tokio::test]
async fn health_and_ready_routes_return_ok() {
    let (state, _database) = common::setup().await;
    let app = create_test_app!(state);
    assert_eq!(
        request(&app, "GET", "/health").await.status(),
        StatusCode::OK
    );
    assert_eq!(
        json(request(&app, "GET", "/health/ready").await).await["status"],
        "ready"
    );
}

#[tokio::test]
async fn api_scope_returns_json_404() {
    let (state, _database) = common::setup().await;
    let app = create_test_app!(state);
    let response = request(&app, "GET", "/api/v1/missing-route").await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    assert_eq!(json(response).await["code"], "endpoint_not_found");
}

#[tokio::test]
async fn frontend_fallback_serves_spa_routes() {
    let (state, _database) = common::setup().await;
    let app = create_test_app!(state);
    let response = request(&app, "GET", "/settings/runtime").await;
    assert_eq!(response.status(), StatusCode::OK);
    assert!(
        response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .is_some_and(|v| v.starts_with("text/html"))
    );
}

#[tokio::test]
async fn middleware_adds_request_id_and_security_headers() {
    let (state, _database) = common::setup().await;
    let app = create_test_app!(state);
    let response = request(&app, "GET", "/health").await;
    assert_eq!(response.status(), StatusCode::OK);
    assert!(response.headers().contains_key("x-request-id"));
    assert_eq!(response.headers()["x-frame-options"], "SAMEORIGIN");
    assert_eq!(response.headers()["x-content-type-options"], "nosniff");
}

#[cfg(feature = "metrics")]
#[tokio::test]
async fn metrics_route_exports_prometheus_text() {
    let _ = aster_forge_metrics::prometheus::init_metrics();
    let (state, _database) = common::setup().await;
    let app = create_test_app!(state);
    let response = request(&app, "GET", "/health/metrics").await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 4 * 1024 * 1024)
        .await
        .expect("metrics body should read");
    assert!(
        std::str::from_utf8(&body)
            .expect("metrics should be utf-8")
            .contains("process_uptime_seconds")
    );
}
