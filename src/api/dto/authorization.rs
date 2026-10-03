use crate::services::authorization::models::SystemPermission;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(utoipa::ToSchema))]
pub enum SystemPermissionDto {
    #[serde(rename = "astergate.system.accounts.read")]
    #[cfg_attr(
        all(debug_assertions, feature = "openapi"),
        schema(rename = "astergate.system.accounts.read")
    )]
    AccountsRead,
    #[serde(rename = "astergate.system.accounts.create")]
    #[cfg_attr(
        all(debug_assertions, feature = "openapi"),
        schema(rename = "astergate.system.accounts.create")
    )]
    AccountsCreate,
    #[serde(rename = "astergate.system.accounts.suspend")]
    #[cfg_attr(
        all(debug_assertions, feature = "openapi"),
        schema(rename = "astergate.system.accounts.suspend")
    )]
    AccountsSuspend,
    #[serde(rename = "astergate.system.accounts.restore")]
    #[cfg_attr(
        all(debug_assertions, feature = "openapi"),
        schema(rename = "astergate.system.accounts.restore")
    )]
    AccountsRestore,
    #[serde(rename = "astergate.system.accounts.delete")]
    #[cfg_attr(
        all(debug_assertions, feature = "openapi"),
        schema(rename = "astergate.system.accounts.delete")
    )]
    AccountsDelete,
    #[serde(rename = "astergate.system.settings.read")]
    #[cfg_attr(
        all(debug_assertions, feature = "openapi"),
        schema(rename = "astergate.system.settings.read")
    )]
    SettingsRead,
    #[serde(rename = "astergate.system.settings.update")]
    #[cfg_attr(
        all(debug_assertions, feature = "openapi"),
        schema(rename = "astergate.system.settings.update")
    )]
    SettingsUpdate,
}
impl From<SystemPermission> for SystemPermissionDto {
    fn from(value: SystemPermission) -> Self {
        match value {
            SystemPermission::AccountsRead => Self::AccountsRead,
            SystemPermission::AccountsCreate => Self::AccountsCreate,
            SystemPermission::AccountsSuspend => Self::AccountsSuspend,
            SystemPermission::AccountsRestore => Self::AccountsRestore,
            SystemPermission::AccountsDelete => Self::AccountsDelete,
            SystemPermission::SettingsRead => Self::SettingsRead,
            SystemPermission::SettingsUpdate => Self::SettingsUpdate,
        }
    }
}
