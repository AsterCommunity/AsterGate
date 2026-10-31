use crate::{
    runtime::AppState,
    services::account::{self, AccountError, models::Principal},
};
use axum::{
    Json,
    extract::{FromRequest, FromRequestParts, Request, State},
    http::{HeaderMap, StatusCode, header, request::Parts},
    middleware::Next,
    response::{IntoResponse, Response},
};
use serde::de::DeserializeOwned;
use std::sync::Arc;

impl IntoResponse for AccountError {
    fn into_response(self) -> Response {
        let code = crate::api::api_error_code::ApiErrorCode::from(&self);
        let status = code.status();
        let mut response = code.into_response();
        if status == StatusCode::TOO_MANY_REQUESTS {
            response.headers_mut().insert(
                header::RETRY_AFTER,
                axum::http::HeaderValue::from_static("900"),
            );
        }
        response
    }
}

pub struct JsonInput<T>(pub T);
impl<T: DeserializeOwned + Send> FromRequest<Arc<AppState>> for JsonInput<T> {
    type Rejection = AccountError;
    async fn from_request(
        request: Request,
        state: &Arc<AppState>,
    ) -> Result<Self, Self::Rejection> {
        Json::<T>::from_request(request, state)
            .await
            .map(|v| Self(v.0))
            .map_err(|_| AccountError::InvalidInput)
    }
}

pub struct Authenticated(pub Principal);
impl FromRequestParts<Arc<AppState>> for Authenticated {
    type Rejection = AccountError;
    async fn from_request_parts(
        parts: &mut Parts,
        state: &Arc<AppState>,
    ) -> Result<Self, Self::Rejection> {
        let policy = parts
            .extensions
            .get::<SessionCookiePolicy>()
            .ok_or(AccountError::Unauthenticated)?;
        let mut values = Vec::new();
        for value in parts.headers.get_all(header::COOKIE) {
            let value = value.to_str().map_err(|_| AccountError::Unauthenticated)?;
            for cookie in cookie::Cookie::split_parse(value) {
                let cookie = cookie.map_err(|_| AccountError::Unauthenticated)?;
                if cookie.name() == policy.name() {
                    values.push(cookie.value().to_string());
                }
            }
        }
        if values.len() != 1 {
            return Err(AccountError::Unauthenticated);
        }
        account::authenticate(state, &values[0]).await.map(Self)
    }
}

pub async fn boundary(
    State(state): State<Arc<AppState>>,
    mut request: Request,
    next: Next,
) -> Response {
    use aster_forge_middleware::axum::csrf::{
        CsrfConfig, CsrfTokenNames, RequestSourceMode, RequestSourcePolicy, csrf,
    };
    let safe = matches!(
        *request.method(),
        axum::http::Method::GET | axum::http::Method::HEAD | axum::http::Method::OPTIONS
    );
    let bootstrap = safe
        && matches!(
            request.uri().path(),
            "/setup/status"
                | "/api/v1/setup/status"
                | "/public/frontend-config"
                | "/api/v1/public/frontend-config"
        );
    let headers = request.headers().clone();
    let path = request.uri().path().to_owned();
    let validation = async {
        // Forge's adapters consume single header values. Reject ambiguity before delegating.
        for name in ["host", "origin", "referer", "sec-fetch-site"] {
            if headers.get_all(name).iter().count() > 1 {
                return Err(AccountError::Forbidden);
            }
        }
        if !safe
            && (headers.get_all("x-astergate-request").iter().count() != 1
                || headers
                    .get("x-astergate-request")
                    .and_then(|v| v.to_str().ok())
                    != Some("1"))
        {
            return Err(AccountError::Forbidden);
        }
        if !matches!(
            path.as_str(),
            "/setup"
                | "/api/v1/setup"
                | "/setup/status"
                | "/api/v1/setup/status"
                | "/public/frontend-config"
                | "/api/v1/public/frontend-config"
        ) {
            crate::services::system_setup::require_ready(&state).await?;
        }
        let policy = match state.runtime_config.authentication() {
            Ok(settings) => {
                let mut headers = headers.clone();
                if safe {
                    headers.remove(header::ORIGIN);
                }
                SessionCookiePolicy::for_request(&settings, &headers)?
            }
            Err(AccountError::SetupRequired) => {
                SessionCookiePolicy::before_setup(&headers, &state.config)?
            }
            // Public branding/status remain readable when authentication policy is damaged,
            // but cannot mint a cookie from a fallback security policy.
            Err(_) if bootstrap => return Ok((None, false)),
            Err(error) => return Err(error),
        };
        let token = csrf_cookie_value(&headers, policy.csrf_name())?;
        if !safe {
            let headers = headers.get_all(CSRF_HEADER);
            if token.is_none()
                || headers.iter().count() != 1
                || !headers
                    .iter()
                    .all(|value| value.to_str().is_ok_and(valid_csrf_token))
            {
                return Err(AccountError::Forbidden);
            }
        }
        Ok::<_, AccountError>((Some(policy), token.is_none()))
    }
    .await;
    let (policy, missing_token) = match validation {
        Ok(value) => value,
        Err(error) => return error.into_response(),
    };
    let Some(policy) = policy else {
        return next.run(request).await;
    };
    request.extensions_mut().insert(policy.clone());
    let source = RequestSourcePolicy {
        scheme: if policy.secure { "https" } else { "http" }.into(),
        host: policy.authority.clone(),
        public_site_origins: Vec::new(),
        mode: RequestSourceMode::OptionalWhenPresent,
    };
    // Public cross-origin reads must not replace a host's CSRF cookie.
    let may_bootstrap = bootstrap
        && missing_token
        && aster_forge_middleware::axum::csrf::ensure_request_source_allowed(&headers, &source)
            .is_ok();
    let names = match CsrfTokenNames::new(policy.csrf_name(), CSRF_HEADER) {
        Ok(names) => names,
        Err(_) => return AccountError::Security.into_response(),
    };
    let config = CsrfConfig::new(
        names,
        |_| true,
        move |_| Ok(source.clone()),
        |_| AccountError::Forbidden.into_response(),
    );
    let mut response = csrf(State(config), request, next).await;
    // A handler's session Set-Cookie proves its transaction committed. Rotate alongside it.
    let session_update = response
        .headers()
        .get_all(header::SET_COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .filter_map(|value| cookie::Cookie::parse(value).ok())
        .find(|value| matches!(value.name(), "ag_session" | "__Host-ag_session"))
        .map(|value| {
            (
                value.name() == "__Host-ag_session",
                value.value().is_empty(),
                value.max_age().map(|duration| duration.whole_seconds()),
            )
        });
    if response.status().is_success() && (session_update.is_some() || may_bootstrap) {
        let mut cookie_policy = policy;
        let clear = if let Some((secure, clear, lifetime)) = session_update {
            cookie_policy.secure = secure;
            if let Some(lifetime) = lifetime {
                cookie_policy.lifetime = lifetime;
            }
            clear
        } else {
            false
        };
        let token = if clear {
            String::new()
        } else {
            aster_forge_middleware::shared::csrf::build_csrf_token()
        };
        match csrf_cookie(&cookie_policy, &token) {
            Ok(cookie) => {
                response.headers_mut().append(header::SET_COOKIE, cookie);
            }
            Err(error) => return error.into_response(),
        }
    }
    response
}

pub const CSRF_HEADER: &str = "x-astergate-csrf";

fn valid_csrf_token(value: &str) -> bool {
    (32..=128).contains(&value.len())
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

fn csrf_cookie_value(headers: &HeaderMap, name: &str) -> Result<Option<String>, AccountError> {
    let mut token = None;
    for header in headers.get_all(header::COOKIE) {
        let header = header.to_str().map_err(|_| AccountError::Forbidden)?;
        for value in cookie::Cookie::split_parse(header) {
            let value = value.map_err(|_| AccountError::Forbidden)?;
            if value.name() == name {
                if token.is_some() || !valid_csrf_token(value.value()) {
                    return Err(AccountError::Forbidden);
                }
                token = Some(value.value().to_owned());
            }
        }
    }
    Ok(token)
}

/// Outer response policy also covers CORS/limiter rejections before the account boundary.
pub async fn response_headers(request: Request, next: Next) -> Response {
    let protected = request.uri().path().starts_with("/api/v1/");
    let mut response = next.run(request).await;
    if !protected {
        return response;
    }
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        axum::http::HeaderValue::from_static("no-store"),
    );
    response.headers_mut().insert(
        header::PRAGMA,
        axum::http::HeaderValue::from_static("no-cache"),
    );
    response.headers_mut().insert(
        header::REFERRER_POLICY,
        axum::http::HeaderValue::from_static("no-referrer"),
    );
    response
}

#[derive(Clone)]
pub struct SessionCookiePolicy {
    secure: bool,
    lifetime: i64,
    authority: String,
}
impl SessionCookiePolicy {
    pub fn for_request(
        settings: &crate::services::system_settings::models::AuthenticationSettings,
        headers: &HeaderMap,
    ) -> Result<Self, AccountError> {
        use crate::services::system_settings::site::PublicSiteOrigin;
        // Only configured authorities can determine cookie security. Forwarded headers are never trusted.
        let host_site = if let Some(host) = headers.get(header::HOST) {
            let host = host.to_str().map_err(|_| AccountError::Forbidden)?;
            Some(
                settings
                    .site_urls
                    .iter()
                    .find(|site| site.authority() == host)
                    .ok_or(AccountError::Forbidden)?,
            )
        } else {
            None
        };
        let origin_site = if let Some(origin) = headers.get(header::ORIGIN) {
            let origin = origin.to_str().map_err(|_| AccountError::Forbidden)?;
            Some(
                settings
                    .site_urls
                    .find_browser_origin(origin)
                    .ok_or(AccountError::Forbidden)?,
            )
        } else {
            None
        };
        if let (Some(host), Some(origin)) = (host_site, origin_site)
            && host != origin
        {
            return Err(AccountError::Forbidden);
        }
        let site: &PublicSiteOrigin = host_site
            .or(origin_site)
            .unwrap_or_else(|| settings.site_urls.primary());
        Ok(Self {
            secure: site.secure_cookies(),
            lifetime: settings.session_lifetime.seconds(),
            authority: site.authority().to_owned(),
        })
    }
    pub(super) fn before_setup(
        headers: &HeaderMap,
        config: &crate::config::AppConfig,
    ) -> Result<Self, AccountError> {
        use crate::services::system_settings::site::PublicSiteOrigin;
        let fallback = format!("{}:{}", config.server.host, config.server.port);
        let host = headers
            .get(header::HOST)
            .map(|value| value.to_str())
            .transpose()
            .map_err(|_| AccountError::Forbidden)?
            .unwrap_or(&fallback);
        let site = PublicSiteOrigin::parse(&format!("http://{host}"))
            .or_else(|_| PublicSiteOrigin::parse(&format!("https://{host}")))
            .map_err(|_| AccountError::Forbidden)?;
        if site.authority() != host {
            return Err(AccountError::Forbidden);
        }
        Ok(Self {
            secure: site.secure_cookies(),
            lifetime: 86400,
            authority: host.to_owned(),
        })
    }
    fn csrf_name(&self) -> &'static str {
        if self.secure {
            "__Host-ag_csrf"
        } else {
            "ag_csrf"
        }
    }
    pub(super) fn origin(&self) -> String {
        format!(
            "{}://{}",
            if self.secure { "https" } else { "http" },
            self.authority
        )
    }
    fn name(&self) -> &'static str {
        if self.secure {
            "__Host-ag_session"
        } else {
            "ag_session"
        }
    }
}

fn csrf_cookie(
    policy: &SessionCookiePolicy,
    value: &str,
) -> Result<axum::http::HeaderValue, AccountError> {
    cookie::Cookie::build((policy.csrf_name(), value.to_string()))
        .path("/")
        .http_only(false)
        .secure(policy.secure)
        .same_site(cookie::SameSite::Strict)
        .max_age(cookie::time::Duration::seconds(if value.is_empty() {
            0
        } else {
            policy.lifetime
        }))
        .build()
        .to_string()
        .parse()
        .map_err(|_| AccountError::Security)
}

pub fn session_cookie(
    policy: &SessionCookiePolicy,
    value: &str,
) -> Result<axum::http::HeaderValue, AccountError> {
    let max_age = if value.is_empty() { 0 } else { policy.lifetime };
    cookie::Cookie::build((policy.name(), value.to_string()))
        .path("/")
        .http_only(true)
        .secure(policy.secure)
        .same_site(cookie::SameSite::Lax)
        .max_age(cookie::time::Duration::seconds(max_age))
        .build()
        .to_string()
        .parse()
        .map_err(|_| AccountError::Security)
}
