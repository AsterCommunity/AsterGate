use crate::{
    api::{
        account_security::{Authenticated, JsonInput, SessionCookiePolicy, session_cookie},
        dto::{account::AccountResponse, system::*},
    },
    runtime::AppState,
    services::{
        account::{AccountError, Result},
        system_setup,
    },
};
use axum::{
    Json,
    extract::State,
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
};
use std::sync::Arc;

#[aster_forge_api_docs_macros::path(get, path = "/api/v1/setup/status", tag = "setup", responses((status = 200, body = SetupStatusResponse)))]
pub async fn status(State(state): State<Arc<AppState>>) -> Result<Json<SetupStatusResponse>> {
    Ok(Json(SetupStatusResponse::ok(
        system_setup::status(&state).await?.into(),
    )))
}
#[aster_forge_api_docs_macros::path(post, path = "/api/v1/setup", tag = "setup", request_body = SetupRequest, responses((status = 201, body = AccountResponse), (status = 409, body = crate::api::response::ErrorResponse)))]
pub async fn initialize(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    JsonInput(input): JsonInput<SetupRequest>,
) -> Result<Response> {
    let command = input.command()?;
    let cookie_policy =
        SessionCookiePolicy::for_request(&command.settings.authentication(), &headers)?;
    let user_agent = headers
        .get(header::USER_AGENT)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let result = system_setup::initialize(&state, command, user_agent).await?;
    let mut response = (
        StatusCode::CREATED,
        Json(AccountResponse::ok(result.account.into())),
    )
        .into_response();
    response.headers_mut().insert(
        header::SET_COOKIE,
        session_cookie(&cookie_policy, &result.session_token)?,
    );
    Ok(response)
}
#[aster_forge_api_docs_macros::path(get, path = "/api/v1/management/settings", tag = "instance-settings", responses((status = 200, body = SettingsResponse)))]
pub async fn settings(
    State(state): State<Arc<AppState>>,
    Authenticated(operator): Authenticated,
) -> Result<Json<SettingsResponse>> {
    Ok(Json(SettingsResponse::ok(
        system_setup::read_settings(&state, &operator).await?.into(),
    )))
}
#[aster_forge_api_docs_macros::path(put, path = "/api/v1/management/settings", tag = "instance-settings", request_body = SettingsRequest, responses((status = 200, body = SettingsResponse)))]
pub async fn update_settings(
    State(state): State<Arc<AppState>>,
    Authenticated(operator): Authenticated,
    JsonInput(mut input): JsonInput<SettingsRequest>,
) -> Result<Json<SettingsResponse>> {
    let previous = system_setup::settings_for_update(&state, &operator).await?;
    if previous.revision != input.revision {
        return Err(AccountError::SettingsConflict);
    }
    let password = input.current_password.take();
    let settings = input.command(&previous)?;
    Ok(Json(SettingsResponse::ok(
        system_setup::update_settings(&state, &operator, settings, password)
            .await?
            .into(),
    )))
}
