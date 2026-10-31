//! Typed HTTP input and authoritative Account authentication.
use super::cookies::SessionCookiePolicy;
use crate::{
    runtime::AppState,
    services::account::{self, AccountError, models::Principal},
};
use axum::{
    Json,
    extract::{FromRequest, FromRequestParts, Request},
    http::{header, request::Parts},
};
use serde::de::DeserializeOwned;
use std::sync::Arc;

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
