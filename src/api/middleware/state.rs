//! Instance-owned HTTP state. Cloning/reconstructing routers never resets quotas.
use crate::{
    config::transport::{CorsConfig, HttpSecurityConfig},
    errors::AppError,
};
use aster_forge_middleware::axum::{cors, rate_limit};
use std::sync::{
    RwLock,
    atomic::{AtomicUsize, Ordering},
};

pub struct HttpSecurity {
    pub(super) ip: rate_limit::NormalizedStringRateLimiter,
    pub(super) authentication: rate_limit::NormalizedStringRateLimiter,
    pub(super) cors: RwLock<cors::RuntimeCorsPolicy>,
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
            ip: rate_limit::NormalizedStringRateLimiter::new(
                limits.enabled,
                limits.seconds_per_request,
                limits.burst,
            ),
            authentication: rate_limit::NormalizedStringRateLimiter::new(
                limits.enabled,
                limits.authentication_seconds_per_request,
                limits.authentication_burst,
            ),
            cors: RwLock::new(super::cors::policy(&config.cors)?),
            requests: AtomicUsize::new(0),
        })
    }

    /// Atomically replaces a validated policy; each request resolves the current snapshot.
    pub fn replace_cors(&self, config: &CorsConfig) -> Result<(), AppError> {
        let policy = super::cors::policy(config)?;
        *self
            .cors
            .write()
            .map_err(|_| AppError::Config("HTTP CORS policy unavailable".into()))? = policy;
        Ok(())
    }

    pub(super) fn maintain(&self) {
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
