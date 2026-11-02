//! Deployment-owned HTTP policy. Public sites never implicitly enable cross-origin access.

#[derive(Debug, Clone, Default, serde::Deserialize, serde::Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct HttpSecurityConfig {
    pub trusted_proxies: Vec<String>,
    pub cors: CorsConfig,
    pub rate_limit: HttpRateLimitConfig,
}

#[derive(Debug, Clone, Default, serde::Deserialize, serde::Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct CorsConfig {
    pub enabled: bool,
    pub allowed_origins: Vec<String>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct HttpRateLimitConfig {
    pub enabled: bool,
    pub seconds_per_request: std::num::NonZeroU64,
    pub burst: std::num::NonZeroU32,
    pub authentication_seconds_per_request: std::num::NonZeroU64,
    pub authentication_burst: std::num::NonZeroU32,
}

impl Default for HttpRateLimitConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            seconds_per_request: std::num::NonZeroU64::MIN,
            burst: std::num::NonZeroU32::new(300).unwrap_or(std::num::NonZeroU32::MIN),
            authentication_seconds_per_request: std::num::NonZeroU64::new(2)
                .unwrap_or(std::num::NonZeroU64::MIN),
            authentication_burst: std::num::NonZeroU32::new(100)
                .unwrap_or(std::num::NonZeroU32::MIN),
        }
    }
}
