//! Gate's ready-state and CSRF policy over Forge, installed on explicit route groups.
use crate::api::cookies::{self, SessionCookiePolicy};
use crate::{runtime::AppState, services::account::AccountError};
use axum::{
    extract::{Request, State},
    http::{HeaderMap, header},
    middleware::Next,
    response::{IntoResponse, Response},
};
use std::sync::Arc;

/// Account/auth/management routes require completed initialization; bootstrap/setup do not.
pub async fn require_ready(
    State(state): State<Arc<AppState>>,
    request: Request,
    next: Next,
) -> Response {
    match crate::services::system_setup::require_ready(&state).await {
        Ok(()) => next.run(request).await,
        Err(error) => error.into_response(),
    }
}

/// Protect ordinary Account/setup routes without anonymous Cookie issuance.
pub async fn protect(State(state): State<Arc<AppState>>, request: Request, next: Next) -> Response {
    protect_request(state, request, next, false).await
}

/// Only the explicit public bootstrap group can mint tokens and tolerate damaged auth policy.
pub async fn bootstrap(
    State(state): State<Arc<AppState>>,
    request: Request,
    next: Next,
) -> Response {
    protect_request(state, request, next, true).await
}

async fn protect_request(
    state: Arc<AppState>,
    mut request: Request,
    next: Next,
    bootstrap: bool,
) -> Response {
    use aster_forge_middleware::axum::csrf::{CsrfConfig, CsrfTokenNames, csrf};
    let safe = matches!(
        *request.method(),
        axum::http::Method::GET | axum::http::Method::HEAD | axum::http::Method::OPTIONS
    );
    let headers = request.headers().clone();
    let validation = (|| {
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
            Err(_) if safe && bootstrap => return Ok(None),
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
        Ok::<_, AccountError>(Some((policy, token.is_none())))
    })();
    let policy = match validation {
        Ok(value) => value,
        Err(error) => return error.into_response(),
    };
    let Some((policy, missing_token)) = policy else {
        return next.run(request).await;
    };
    request.extensions_mut().insert(policy.clone());
    let source = policy.source();
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
    let may_bootstrap = bootstrap
        && matches!(
            *request.method(),
            axum::http::Method::GET | axum::http::Method::HEAD
        )
        && missing_token
        && aster_forge_middleware::axum::csrf::ensure_request_source_allowed(
            &headers,
            &policy.source(),
        )
        .is_ok();
    let mut response = csrf(State(config), request, next).await;
    if may_bootstrap
        && response.status().is_success()
        && let Err(error) = cookies::bootstrap_csrf(&mut response, &policy)
    {
        return error.into_response();
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
