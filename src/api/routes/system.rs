use crate::api::{api_error_code::ApiErrorCode, response::NoData};
use crate::{
    api::{
        cookies::{self, SessionCookiePolicy},
        dto::{account::AccountDto, system::*},
        extractors::{Authenticated, JsonInput},
    },
    runtime::AppState,
    services::{
        account::{AccountError, Result},
        system_setup,
    },
};
use aster_forge_api::response::ApiResponse;
use axum::{
    Json,
    extract::State,
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
};
use std::sync::Arc;

#[aster_forge_api_docs_macros::path(get, path = "/api/v1/setup/status", tag = "setup", responses((status = 200, body = ApiResponse<SetupStateDto, ApiErrorCode, NoData>)))]
pub async fn status(
    State(state): State<Arc<AppState>>,
) -> Result<Json<ApiResponse<SetupStateDto, ApiErrorCode, NoData>>> {
    Ok(Json(crate::api::response::success::<SetupStateDto>(
        system_setup::status(&state).await?.into(),
    )?))
}
#[aster_forge_api_docs_macros::path(post, path = "/api/v1/setup", tag = "setup", request_body = SetupRequest, responses((status = 201, body = ApiResponse<AccountDto, ApiErrorCode, NoData>), (status = 409, body = ApiResponse<NoData, ApiErrorCode, NoData>)))]
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
        Json(crate::api::response::success::<AccountDto>(
            result.account.into(),
        )?),
    )
        .into_response();
    cookies::issue_session(&mut response, &cookie_policy, &result.tokens)?;
    Ok(response)
}
#[aster_forge_api_docs_macros::path(get, path = "/api/v1/management/settings", tag = "instance-settings", responses((status = 200, body = ApiResponse<SettingsDto, ApiErrorCode, NoData>)))]
pub async fn settings(
    State(state): State<Arc<AppState>>,
    Authenticated(operator): Authenticated,
) -> Result<Json<ApiResponse<SettingsDto, ApiErrorCode, NoData>>> {
    Ok(Json(crate::api::response::success::<SettingsDto>(
        system_setup::read_settings(&state, &operator).await?.into(),
    )?))
}
#[aster_forge_api_docs_macros::path(put, path = "/api/v1/management/settings", tag = "instance-settings", request_body = SettingsRequest, responses((status = 200, body = ApiResponse<SettingsDto, ApiErrorCode, NoData>)))]
pub async fn update_settings(
    State(state): State<Arc<AppState>>,
    Authenticated(operator): Authenticated,
    JsonInput(mut input): JsonInput<SettingsRequest>,
) -> Result<Json<ApiResponse<SettingsDto, ApiErrorCode, NoData>>> {
    let previous = system_setup::settings_for_update(&state, &operator).await?;
    if previous.revision != input.revision {
        return Err(AccountError::SettingsConflict);
    }
    let password = input.current_password.take();
    let settings = input.command(&previous)?;
    Ok(Json(crate::api::response::success::<SettingsDto>(
        system_setup::update_settings(&state, &operator, settings, password)
            .await?
            .into(),
    )?))
}
