//! Charge admission before CORS (including preflight), enforce before product handlers.
use crate::{runtime::AppState, services::account::AccountError};
use aster_forge_middleware::axum::{client_ip, rate_limit::RateLimitRejection};
use axum::{
    extract::{Request, State},
    http::{HeaderValue, header},
    middleware::Next,
    response::{IntoResponse, Response},
};
use std::{
    net::{IpAddr, Ipv4Addr},
    sync::Arc,
};

/// Compute a Forge-owned quota decision once, even when CORS short-circuits preflight.
/// Rejected actual requests still reach CORS response handling, never a product handler.
pub async fn admission(
    State(state): State<Arc<AppState>>,
    mut request: Request,
    next: Next,
) -> Response {
    if !request.uri().path().starts_with("/api/v1/") {
        return next.run(request).await;
    }
    state.http_security.maintain();
    let ip = client_ip::direct_peer(&request).map(|peer| {
        client_ip::real_ip_from_headers(
            request.headers(),
            peer,
            &state.config.http_security.trusted_proxies,
        )
    });
    let auth = request.uri().path().starts_with("/api/v1/auth/")
        || request.uri().path() == "/api/v1/setup";
    let rejection = if auth {
        let key = ip.map_or_else(|| "unknown-peer".into(), |ip| ip.to_string());
        state.http_security.authentication.check(&key)
    } else {
        None
    }
    .or_else(|| {
        // Match Forge's IP adapter: missing transport peers share the localhost bucket.
        state
            .http_security
            .ip
            .check(&ip.unwrap_or(IpAddr::V4(Ipv4Addr::LOCALHOST)).to_string())
    });
    if let Some(rejection) = rejection {
        request.extensions_mut().insert(rejection);
    }
    let response = next.run(request).await;
    if let Some(rejection) = rejection {
        // Preflight/denial may return directly from CORS, skipping enforce(). Preserve only
        // its CORS headers, never its 204 status/body or a handler's content metadata.
        let mut limited = rate_limit_response(rejection);
        for (name, value) in response.headers() {
            if name == header::VARY || name.as_str().starts_with("access-control-") {
                limited.headers_mut().append(name.clone(), value.clone());
            }
        }
        limited
    } else {
        response
    }
}

/// Installed inside CORS so actual 429s acquire exactly its normal allowlist headers.
pub async fn enforce(request: Request, next: Next) -> Response {
    if let Some(rejection) = request.extensions().get::<RateLimitRejection>() {
        rate_limit_response(*rejection)
    } else {
        next.run(request).await
    }
}

fn rate_limit_response(rejection: RateLimitRejection) -> Response {
    let mut response = AccountError::RateLimited.into_response();
    if let Ok(value) = HeaderValue::from_str(&rejection.retry_after_seconds().to_string()) {
        response.headers_mut().insert(header::RETRY_AFTER, value);
    } else {
        return AccountError::Security.into_response();
    }
    response
}
