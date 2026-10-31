//! Dynamic CORS policy and wire mapping over the Forge Axum adapter.
use crate::{
    config::transport::CorsConfig,
    errors::AppError,
    runtime::AppState,
    services::{account::AccountError, system_settings::site::PublicSiteOrigin},
};
use aster_forge_middleware::axum::cors;
use axum::{
    extract::{Request, State},
    http::header,
    middleware::Next,
    response::{IntoResponse, Response},
};
use std::sync::Arc;

pub(super) fn policy(config: &CorsConfig) -> Result<cors::RuntimeCorsPolicy, AppError> {
    if config.allowed_origins.len() > 64 || (config.enabled && config.allowed_origins.is_empty()) {
        return Err(AppError::Config(
            "enabled CORS requires an explicit bounded origin allowlist".into(),
        ));
    }
    let mut origins = Vec::new();
    for raw in &config.allowed_origins {
        let origin = PublicSiteOrigin::parse(raw)
            .map_err(|_| AppError::Config("invalid HTTP CORS origin".into()))?
            .browser_origin();
        if !origins.contains(&origin) {
            origins.push(origin);
        }
    }
    Ok(cors::RuntimeCorsPolicy {
        enabled: config.enabled,
        allowed_origins: cors::CorsAllowedOrigins::List(origins),
        // Cross-origin cookies are deliberately unsupported. Gate's hosted UI remains same-origin.
        allow_credentials: false,
        max_age_secs: 600,
    })
}

pub fn cors_config(state: Arc<AppState>) -> cors::RuntimeCorsConfig {
    let origin_state = state.clone();
    cors::RuntimeCorsConfig::new(
        move |_| {
            state
                .http_security
                .cors
                .read()
                .map(|policy| policy.clone())
                .map_err(|_| Box::new(AccountError::Storage.into_response()))
        },
        |path| !path.starts_with("/api/v1/"),
        |error| match error.kind() {
            cors::CorsMiddlewareErrorKind::InvalidRequest => {
                AccountError::Forbidden.into_response()
            }
            cors::CorsMiddlewareErrorKind::InvalidResponse => {
                AccountError::Security.into_response()
            }
        },
    )
    .rejection_response(|_| AccountError::Forbidden.into_response())
    .request_origin(move |request| {
        use crate::api::cookies::SessionCookiePolicy;
        let mut headers = request.headers().clone();
        headers.remove(header::ORIGIN);
        let policy = match origin_state.runtime_config.authentication() {
            Ok(settings) => SessionCookiePolicy::for_request(&settings, &headers),
            Err(AccountError::SetupRequired) => {
                SessionCookiePolicy::before_setup(request.headers(), &origin_state.config)
            }
            Err(_) => return Ok(None),
        };
        // Configured Host determines a same-origin bypass; Origin never expands the CORS list.
        policy
            .map(|policy| Some(policy.origin()))
            .map_err(|error| Box::new(error.into_response()))
    })
    .allowed_methods(["GET", "HEAD", "POST", "PUT", "PATCH", "DELETE", "OPTIONS"])
    .allowed_headers([
        "accept",
        "content-type",
        "x-astergate-request",
        "x-astergate-csrf",
    ])
    .exposed_headers(["retry-after", "x-request-id"])
}

/// Reject ambiguous headers before delegating CORS decisions and response mapping to Forge.
pub async fn cors_boundary(
    config: State<cors::RuntimeCorsConfig>,
    request: Request,
    next: Next,
) -> Response {
    if !request.uri().path().starts_with("/api/v1/") {
        return next.run(request).await;
    }
    if [
        "host",
        "origin",
        "access-control-request-method",
        "access-control-request-headers",
    ]
    .iter()
    .any(|name| request.headers().get_all(*name).iter().count() > 1)
    {
        return AccountError::Forbidden.into_response();
    }
    cors::runtime_cors(config, request, next).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        Router,
        body::{Body, to_bytes},
        http::{Request, StatusCode},
        middleware,
        routing::get,
    };
    use tower::ServiceExt;

    #[tokio::test]
    async fn explicit_denial_mapping_does_not_rewrite_downstream_empty_forbidden() {
        let allowed = "https://client.example.test";
        let config = cors::RuntimeCorsConfig::new(
            move |_| {
                Ok(cors::RuntimeCorsPolicy {
                    enabled: true,
                    allowed_origins: cors::CorsAllowedOrigins::List(vec![allowed.into()]),
                    allow_credentials: false,
                    max_age_secs: 600,
                })
            },
            |_| false,
            |_| AccountError::Security.into_response(),
        )
        .rejection_response(|_| AccountError::Forbidden.into_response());
        let app = Router::new()
            .route("/api/v1/test", get(|| async { StatusCode::FORBIDDEN }))
            .layer(middleware::from_fn_with_state(config, cors_boundary));
        for origin in [allowed, "https://denied.example.test"] {
            let response = app
                .clone()
                .oneshot(
                    Request::builder()
                        .uri("/api/v1/test")
                        .header(header::ORIGIN, origin)
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::FORBIDDEN);
            assert!(response.headers().contains_key(header::VARY));
            assert_eq!(
                response
                    .headers()
                    .contains_key(header::ACCESS_CONTROL_ALLOW_ORIGIN),
                origin == allowed
            );
            let body = to_bytes(response.into_body(), 4096).await.unwrap();
            if origin == allowed {
                assert!(body.is_empty());
            } else {
                let body: serde_json::Value = serde_json::from_slice(&body).unwrap();
                assert_eq!(body["code"], "auth.forbidden");
            }
        }
    }
}
