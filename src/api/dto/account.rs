use crate::services::account::models;
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(utoipa::ToSchema))]
pub struct RegisterRequest {
    pub username: String,
    pub email: String,
    pub display_name: String,
    pub password: String,
}
impl TryFrom<RegisterRequest> for models::RegisterAccount {
    type Error = crate::services::account::AccountError;
    fn try_from(v: RegisterRequest) -> Result<Self, Self::Error> {
        Self::parse(&v.username, &v.email, &v.display_name, v.password)
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(utoipa::ToSchema))]
pub struct LoginRequest {
    pub identifier: String,
    pub password: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(utoipa::ToSchema))]
pub struct IdentifierRequest {
    pub identifier: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(utoipa::ToSchema))]
pub struct TokenRequest {
    pub token: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(utoipa::ToSchema))]
pub struct ResetPasswordRequest {
    pub token: String,
    pub password: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(utoipa::ToSchema))]
pub struct ProfileRequest {
    pub display_name: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(utoipa::ToSchema))]
pub struct PasswordRequest {
    pub current_password: String,
    pub new_password: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(utoipa::ToSchema))]
pub struct DeleteAccountRequest {
    pub password: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(utoipa::ToSchema))]
pub struct EmailChangeRequest {
    pub email: String,
    pub password: String,
}

#[derive(Serialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(utoipa::ToSchema))]
pub struct AccountDto {
    pub id: String,
    pub username: String,
    pub email: String,
    pub email_verified: bool,
    pub display_name: String,
    pub status: AccountStatusDto,
    pub created_at: i64,
    pub updated_at: i64,
    pub system_permissions: Vec<super::authorization::SystemPermissionDto>,
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(utoipa::ToSchema))]
pub enum AccountStatusDto {
    PendingVerification,
    Active,
    Suspended,
    Deleted,
}
impl From<models::Account> for AccountDto {
    fn from(v: models::Account) -> Self {
        Self {
            id: v.id.as_str().into(),
            username: v.username,
            email: v.email,
            email_verified: v.email_verified,
            display_name: v.display_name,
            status: match v.status {
                models::AccountStatus::PendingVerification => AccountStatusDto::PendingVerification,
                models::AccountStatus::Active => AccountStatusDto::Active,
                models::AccountStatus::Suspended => AccountStatusDto::Suspended,
                models::AccountStatus::Deleted => AccountStatusDto::Deleted,
            },
            created_at: v.created_at,
            updated_at: v.updated_at,
            system_permissions: v.system_permissions.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(Serialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(utoipa::ToSchema))]
pub struct SessionDto {
    pub id: String,
    pub current: bool,
    pub created_at: i64,
    pub expires_at: i64,
    pub user_agent: String,
}
impl From<models::Session> for SessionDto {
    fn from(v: models::Session) -> Self {
        Self {
            id: v.id.as_str().into(),
            current: v.current,
            created_at: v.created_at,
            expires_at: v.expires_at,
            user_agent: v.user_agent,
        }
    }
}

crate::api::response::response!(AccountResponse, AccountDto);
crate::api::response::response!(AccountsResponse, aster_forge_api::CursorPage<AccountDto, String>);
crate::api::response::response!(SessionsResponse, Vec<SessionDto>);
crate::api::response::response!(SuccessResponse, bool);
crate::api::response::response!(ExportResponse, AccountExportDto);
#[derive(Serialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(utoipa::ToSchema))]
pub struct AccountExportDto {
    pub account: AccountDto,
    pub sessions: Vec<SessionDto>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AccountListQuery {
    pub after: Option<String>,
    pub limit: Option<u64>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(utoipa::ToSchema))]
pub struct LifecycleRequest {
    pub action: LifecycleActionDto,
    pub current_password: Option<String>,
}
#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(utoipa::ToSchema))]
pub enum LifecycleActionDto {
    Suspend,
    Restore,
    Delete,
}
impl From<LifecycleActionDto> for models::LifecycleAction {
    fn from(v: LifecycleActionDto) -> Self {
        match v {
            LifecycleActionDto::Suspend => Self::Suspend,
            LifecycleActionDto::Restore => Self::Restore,
            LifecycleActionDto::Delete => Self::Delete,
        }
    }
}
