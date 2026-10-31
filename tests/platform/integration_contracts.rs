//! Independent acceptance checks for the envelope/middleware integration.
use super::common;

use aster_gate::{
    api::transport_security::HttpSecurity,
    config::transport::{CorsConfig, HttpRateLimitConfig, HttpSecurityConfig},
};
use axum::{
    Router,
    body::{Body, to_bytes},
    extract::ConnectInfo,
    http::{Request, StatusCode},
};
use serde_json::Value;
use std::{net::SocketAddr, num::NonZeroU32, sync::Arc};
use tower::ServiceExt;

async fn send(
    app: &Router,
    method: &str,
    path: &str,
    origin: Option<&str>,
    peer: &str,
) -> axum::response::Response {
    let mut builder = Request::builder()
        .method(method)
        .uri(path)
        .header("host", "localhost:3000");
    if let Some(origin) = origin {
        builder = builder.header("origin", origin);
    }
    let mut request = builder.body(Body::empty()).unwrap();
    request
        .extensions_mut()
        .insert(ConnectInfo(peer.parse::<SocketAddr>().unwrap()));
    app.clone().oneshot(request).await.unwrap()
}

async fn exercise_admission_chain(app: &Router) {
    let mut request_ids = std::collections::HashSet::new();
    for (method, origin, expected, code) in [
        ("GET", None, StatusCode::OK, "success"),
        (
            "POST",
            Some("https://untrusted.example"),
            StatusCode::FORBIDDEN,
            "auth.forbidden",
        ),
        ("POST", None, StatusCode::FORBIDDEN, "auth.forbidden"),
        ("POST", None, StatusCode::TOO_MANY_REQUESTS, "rate_limited"),
    ] {
        let path = if method == "GET" {
            "/api/v1/setup/status"
        } else {
            "/api/v1/setup"
        };
        let response = send(app, method, path, origin, "198.51.100.1:1234").await;
        assert_eq!(response.status(), expected);
        for (header, value) in [
            ("cache-control", "no-store"),
            ("pragma", "no-cache"),
            ("referrer-policy", "no-referrer"),
            ("x-content-type-options", "nosniff"),
        ] {
            assert_eq!(response.headers()[header], value, "{expected}: {header}");
        }
        assert_eq!(response.headers()["x-frame-options"], "SAMEORIGIN");
        let request_id = response.headers()["x-request-id"].to_str().unwrap();
        uuid::Uuid::parse_str(request_id).unwrap();
        assert!(request_ids.insert(request_id.to_owned()));
        if expected == StatusCode::TOO_MANY_REQUESTS {
            assert!(
                response.headers()["retry-after"]
                    .to_str()
                    .unwrap()
                    .parse::<u64>()
                    .unwrap()
                    > 0
            );
        }
        let body: Value =
            serde_json::from_slice(&to_bytes(response.into_body(), 65536).await.unwrap()).unwrap();
        assert_eq!(body["code"], code);
        assert!(body["msg"].is_string());
        if expected.is_success() {
            assert!(body.get("data").is_some());
            assert!(body.get("error").is_none());
        } else {
            assert!(body.get("data").is_none());
            assert_eq!(
                body["error"]["retryable"],
                expected == StatusCode::TOO_MANY_REQUESTS
            );
            assert!(body["error"].get("diagnostic").is_none());
        }
    }
}

fn configure(state: &mut aster_gate::runtime::AppState) {
    state.http_security = Arc::new(
        HttpSecurity::new(&HttpSecurityConfig {
            cors: CorsConfig {
                enabled: true,
                allowed_origins: vec!["https://trusted.example".into()],
            },
            rate_limit: HttpRateLimitConfig {
                burst: NonZeroU32::new(3).unwrap(),
                ..Default::default()
            },
            ..Default::default()
        })
        .unwrap(),
    );
}

#[tokio::test]
async fn outer_rejections_keep_envelope_headers_and_request_correlation() {
    let (mut state, _database) = common::setup().await;
    configure(&mut state);
    let app = aster_gate::api::router(Arc::new(state));
    exercise_admission_chain(&app).await;
    // Health stays available after exhausting API limits and keeps its protocol shape.
    let response = send(&app, "GET", "/health", None, "198.51.100.1:1234").await;
    assert_eq!(response.status(), StatusCode::OK);
    let value: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 4096).await.unwrap()).unwrap();
    assert_eq!(value["status"], "ok");
    assert!(value.get("code").is_none());
}

#[tokio::test]
async fn cors_preflight_rejects_ambiguous_hosts_without_wrapping_health_routes() {
    let (mut state, _database) = common::setup().await;
    configure(&mut state);
    let app = aster_gate::api::router(Arc::new(state));
    let request = Request::builder()
        .method("OPTIONS")
        .uri("/api/v1/auth/login")
        .header("host", "localhost:3000")
        .header("host", "untrusted.example")
        .header("origin", "https://trusted.example")
        .header("access-control-request-method", "POST")
        .body(Body::empty())
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert_eq!(response.headers()["cache-control"], "no-store");
    assert!(
        !response
            .headers()
            .contains_key("access-control-allow-origin")
    );
    let value: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 4096).await.unwrap()).unwrap();
    assert_eq!(value["code"], "auth.forbidden");
    let request = Request::builder()
        .uri("/health")
        .header("origin", "https://first.example")
        .header("origin", "https://second.example")
        .body(Body::empty())
        .unwrap();
    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let value: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 4096).await.unwrap()).unwrap();
    assert_eq!(value["status"], "ok");
    assert!(value.get("code").is_none());
}

#[cfg(feature = "metrics")]
#[tokio::test]
async fn actual_provider_records_success_csrf_cors_and_limits_without_raw_path_labels() {
    let (mut state, _database) = common::setup().await;
    configure(&mut state);
    let app = aster_gate::api::router(Arc::new(state));
    exercise_admission_chain(&app).await;
    for (method, path, status) in [
        (
            "GET",
            "/api/v1/missing-private-account-id",
            StatusCode::NOT_FOUND,
        ),
        (
            "DELETE",
            "/api/v1/account/sessions/private-session-id",
            StatusCode::FORBIDDEN,
        ),
    ] {
        let response = send(&app, method, path, None, "198.51.100.2:1234").await;
        assert_eq!(response.status(), status);
        to_bytes(response.into_body(), 65536).await.unwrap();
    }
    let response = send(&app, "GET", "/health/metrics", None, "198.51.100.1:1234").await;
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = to_bytes(response.into_body(), 4 * 1024 * 1024)
        .await
        .unwrap();
    let exported = std::str::from_utf8(&bytes).unwrap();
    assert!(
        exported.contains("aster_http_server_requests_total"),
        "{exported}"
    );
    for status in [200, 403, 404, 429] {
        assert!(
            exported.lines().any(|line| !line.starts_with('#')
                && line.contains(&format!("http_response_status_code=\"{status}\""))),
            "missing HTTP {status}: {exported}"
        );
    }
    assert!(
        exported.contains("/api/v1/account/sessions/{id}"),
        "{exported}"
    );
    for secret in [
        "missing-private-account-id",
        "private-session-id",
        "198.51.100.1",
        "untrusted.example",
        "x_request_id",
    ] {
        assert!(!exported.contains(secret), "unbounded label: {secret}");
    }
}
