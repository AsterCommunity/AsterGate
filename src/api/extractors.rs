//! Typed HTTP input and authoritative Account authentication.
use super::cookies::SessionCookiePolicy;
use crate::{
    runtime::AppState,
    services::account::{self, AccountError, models::Principal},
};
use axum::{
    Json,
    extract::{FromRequest, FromRequestParts, Request},
    http::request::Parts,
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
    /// Requires one access Cookie selected by the trusted request policy, then checks writer authority.
    /// Refresh credentials are never accepted as ordinary API authentication.
    async fn from_request_parts(
        parts: &mut Parts,
        state: &Arc<AppState>,
    ) -> Result<Self, Self::Rejection> {
        let policy = parts
            .extensions
            .get::<SessionCookiePolicy>()
            .ok_or(AccountError::Unauthenticated)?;
        let token = super::cookies::credential(&parts.headers, policy.access_name())?
            .ok_or(AccountError::Unauthenticated)?;
        account::sessions::authenticate(state, &token)
            .await
            .map(Self)
    }
}
