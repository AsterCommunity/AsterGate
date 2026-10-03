use super::system::SetupStateDto;
use crate::services::system_settings::branding::PublicBranding;
use serde::Serialize;

#[derive(Serialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(utoipa::ToSchema))]
pub struct PublicBrandingDto {
    pub title: String,
    pub description: String,
}
impl From<PublicBranding> for PublicBrandingDto {
    fn from(value: PublicBranding) -> Self {
        Self {
            title: value.title.as_str().into(),
            description: value.description.as_str().into(),
        }
    }
}

#[derive(Serialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(utoipa::ToSchema))]
pub struct PublicAccountPolicyDto {
    pub setup_state: SetupStateDto,
    pub registration_enabled: bool,
    pub mail_enabled: bool,
    pub email_verification_required: bool,
    pub minimum_password_length: u8,
}

#[derive(Serialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(utoipa::ToSchema))]
pub struct PublicFrontendConfigDto {
    pub branding: PublicBrandingDto,
    pub account: PublicAccountPolicyDto,
}
crate::api::response::response!(PublicFrontendConfigResponse, PublicFrontendConfigDto);
