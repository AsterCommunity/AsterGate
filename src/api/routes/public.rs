//! Anonymous frontend configuration queries, following AsterDrive's public route ownership.
use crate::api::{api_error_code::ApiErrorCode, response::NoData};
use crate::{
    api::dto::public::*,
    runtime::AppState,
    services::{
        account::Result,
        system_settings::{
            models::SetupState,
            policy::{EmailVerificationPolicy, RegistrationPolicy},
        },
        system_setup,
    },
};
use aster_forge_api::response::ApiResponse;
use axum::{Json, Router, extract::State, routing::get};
use std::sync::Arc;

pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/frontend-config", get(frontend_config))
}

#[aster_forge_api_docs_macros::path(get, path = "/api/v1/public/frontend-config", tag = "public", responses((status = 200, body = ApiResponse<PublicFrontendConfigDto, ApiErrorCode, NoData>)))]
pub async fn frontend_config(
    State(state): State<Arc<AppState>>,
) -> Result<Json<ApiResponse<PublicFrontendConfigDto, ApiErrorCode, NoData>>> {
    let setup_state = system_setup::status(&state).await?;
    let settings = if setup_state == SetupState::Ready {
        Some(state.runtime_config.authentication()?)
    } else {
        None
    };
    Ok(Json(crate::api::response::success::<
        PublicFrontendConfigDto,
    >(PublicFrontendConfigDto {
        branding: state.runtime_config.branding()?.into(),
        account: PublicAccountPolicyDto {
            setup_state: setup_state.into(),
            registration_enabled: settings
                .as_ref()
                .is_some_and(|settings| settings.registration == RegistrationPolicy::Open),
            mail_enabled: settings
                .as_ref()
                .is_some_and(|settings| settings.mail_enabled),
            email_verification_required: settings.as_ref().is_some_and(|settings| {
                settings.email_verification == EmailVerificationPolicy::Required
            }),
            minimum_password_length: 12,
        },
    })?))
}
