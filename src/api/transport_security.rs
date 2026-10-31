//! HTTP admission mechanics are Forge-owned; this module supplies Gate policy and error mapping.
use crate::{
    config::transport::{CorsConfig, HttpSecurityConfig},
    errors::AppError,
    runtime::AppState,
    services::account::AccountError,
    services::system_settings::site::PublicSiteOrigin,
};
use aster_forge_middleware::axum::{client_ip, cors, rate_limit};
use axum::{
    extract::{Request, State},
    http::{HeaderValue, StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Response},
};
use std::sync::{
    Arc, RwLock,
    atomic::{AtomicUsize, Ordering},
};

pub struct HttpSecurity {
    ip: rate_limit::IpRateLimitConfig,
    authentication: rate_limit::NormalizedStringRateLimiter,
    cors: RwLock<cors::RuntimeCorsPolicy>,
    requests: AtomicUsize,
}

impl HttpSecurity {
    pub fn new(config: &HttpSecurityConfig) -> Result<Self, AppError> {
        // Forge intentionally tolerates invalid proxy entries; product configuration rejects them.
        if config.trusted_proxies.len() > 64
            || config.trusted_proxies.iter().any(|value| {
                aster_forge_utils::net::parse_trusted_proxies(std::slice::from_ref(value)).len()
                    != 1
            })
        {
            return Err(AppError::Config(
                "invalid HTTP trusted proxy configuration".into(),
            ));
        }
        let limits = &config.rate_limit;
        for (seconds, burst) in [
            (limits.seconds_per_request, limits.burst),
            (
                limits.authentication_seconds_per_request,
                limits.authentication_burst,
            ),
        ] {
            if seconds
                .get()
                .checked_mul(1_000_000_000)
                .and_then(|period| period.checked_mul(u64::from(burst.get())))
                .is_none()
            {
                return Err(AppError::Config(
                    "HTTP rate limit burst window exceeds supported duration".into(),
                ));
            }
        }
        Ok(Self {
            ip: rate_limit::IpRateLimitConfig::new(
                limits.enabled,
                limits.seconds_per_request,
                limits.burst,
                &config.trusted_proxies,
                rate_limit_response,
            ),
            authentication: rate_limit::NormalizedStringRateLimiter::new(
                limits.enabled,
                limits.authentication_seconds_per_request,
                limits.authentication_burst,
            ),
            cors: RwLock::new(cors_policy(&config.cors)?),
            requests: AtomicUsize::new(0),
        })
    }

    /// Atomically replaces a validated policy; each request resolves the current snapshot.
    pub fn replace_cors(&self, config: &CorsConfig) -> Result<(), AppError> {
        let policy = cors_policy(config)?;
        *self
            .cors
            .write()
            .map_err(|_| AppError::Config("HTTP CORS policy unavailable".into()))? = policy;
        Ok(())
    }

    fn maintain(&self) {
        if self
            .requests
            .fetch_add(1, Ordering::Relaxed)
            .is_multiple_of(256)
        {
            self.ip.retain_recent();
            self.authentication.retain_recent();
        }
    }
}

fn cors_policy(config: &CorsConfig) -> Result<cors::RuntimeCorsPolicy, AppError> {
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
        |_| AccountError::Forbidden.into_response(),
    )
    .request_origin(move |request| {
        use crate::api::account_security::SessionCookiePolicy;
        let mut headers = request.headers().clone();
        headers.remove(header::ORIGIN);
        let policy = match origin_state.runtime_config.authentication() {
            Ok(settings) => SessionCookiePolicy::for_request(&settings, &headers),
            Err(AccountError::SetupRequired) => {
                SessionCookiePolicy::before_setup(&headers, &origin_state.config)
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

/// Forge's explicit allowlist denial is a bare 403; add Gate's wire envelope at this boundary.
pub async fn cors_boundary(
    config: State<cors::RuntimeCorsConfig>,
    request: Request,
    next: Next,
) -> Response {
    use axum::body::HttpBody;
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
    let response = cors::runtime_cors(config, request, next).await;
    if response.status() == StatusCode::FORBIDDEN && response.body().size_hint().exact() == Some(0)
    {
        let (parts, _) = response.into_parts();
        let mut mapped = AccountError::Forbidden.into_response();
        mapped.headers_mut().extend(parts.headers);
        mapped
    } else {
        response
    }
}

/// API limits survive Router cloning; process-local buckets do not replace durable Account budgets.
pub async fn admission(
    State(state): State<Arc<AppState>>,
    request: Request,
    next: Next,
) -> Response {
    if !request.uri().path().starts_with("/api/v1/") {
        return next.run(request).await;
    }
    state.http_security.maintain();
    if request.uri().path().starts_with("/api/v1/auth/") || request.uri().path() == "/api/v1/setup"
    {
        let peer = client_ip::direct_peer(&request);
        let ip = peer.map(|peer| {
            client_ip::real_ip_from_headers(
                request.headers(),
                peer,
                &state.config.http_security.trusted_proxies,
            )
        });
        // A missing connection peer is an explicit shared bucket, never inferred from a header.
        let key = ip.map_or_else(|| "unknown-peer".into(), |ip| ip.to_string());
        if let Some(rejection) = state.http_security.authentication.check(&key) {
            return rate_limit_response(rejection);
        }
    }
    rate_limit::ip_rate_limit(State(state.http_security.ip.clone()), request, next).await
}

fn rate_limit_response(rejection: rate_limit::RateLimitRejection) -> Response {
    let mut response = AccountError::RateLimited.into_response();
    if let Ok(value) = HeaderValue::from_str(&rejection.retry_after_seconds().to_string()) {
        response.headers_mut().insert(header::RETRY_AFTER, value);
    } else {
        return AccountError::Security.into_response();
    }
    response
}
