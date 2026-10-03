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
        let mut response = (
            status,
            Json(crate::api::response::ErrorResponse::failure(code)),
        )
            .into_response();
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
    let safe = matches!(
        *request.method(),
        axum::http::Method::GET | axum::http::Method::HEAD | axum::http::Method::OPTIONS
    );
    let validation = async {
        if !safe
            && request
                .headers()
                .get("x-astergate-request")
                .and_then(|v| v.to_str().ok())
                != Some("1")
        {
            return Err(AccountError::Forbidden);
        }
        let path = request.uri().path();
        if !matches!(
            path,
            "/setup"
                | "/api/v1/setup"
                | "/setup/status"
                | "/api/v1/setup/status"
                | "/public/frontend-config"
                | "/api/v1/public/frontend-config"
        ) {
            crate::services::system_setup::require_ready(&state).await?;
            let policy = SessionCookiePolicy::for_request(
                &state.runtime_config.authentication()?,
                request.headers(),
            )?;
            request.extensions_mut().insert(policy);
        }
        Ok::<_, AccountError>(())
    }
    .await;
    let mut response = match validation {
        Ok(()) => next.run(request).await,
        Err(error) => error.into_response(),
    };
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
        })
    }
    fn name(&self) -> &'static str {
        if self.secure {
            "__Host-ag_session"
        } else {
            "ag_session"
        }
    }
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
