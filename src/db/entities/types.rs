use sea_orm::entity::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(24))")]
pub enum StoredAccountStatus {
    #[sea_orm(string_value = "pending_verification")]
    PendingVerification,
    #[sea_orm(string_value = "active")]
    Active,
    #[sea_orm(string_value = "suspended")]
    Suspended,
    #[sea_orm(string_value = "deleted")]
    Deleted,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(16))")]
pub enum StoredIdentifierKind {
    #[sea_orm(string_value = "username")]
    Username,
    #[sea_orm(string_value = "email")]
    Email,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(24))")]
pub enum StoredChallengePurpose {
    #[sea_orm(string_value = "verify_email")]
    VerifyEmail,
    #[sea_orm(string_value = "reset_password")]
    ResetPassword,
    #[sea_orm(string_value = "replace_email")]
    ReplaceEmail,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(128))")]
pub enum SystemPermissionValue {
    #[sea_orm(string_value = "astergate.system.accounts.read")]
    AccountsRead,
    #[sea_orm(string_value = "astergate.system.accounts.create")]
    AccountsCreate,
    #[sea_orm(string_value = "astergate.system.accounts.suspend")]
    AccountsSuspend,
    #[sea_orm(string_value = "astergate.system.accounts.restore")]
    AccountsRestore,
    #[sea_orm(string_value = "astergate.system.accounts.delete")]
    AccountsDelete,
    #[sea_orm(string_value = "astergate.system.settings.read")]
    SettingsRead,
    #[sea_orm(string_value = "astergate.system.settings.update")]
    SettingsUpdate,
}

macro_rules! map_enum {
    ($stored:ident, $domain:ident, $($variant:ident),+) => {
        impl From<crate::services::account::models::$domain> for $stored {
            fn from(value: crate::services::account::models::$domain) -> Self { match value { $(crate::services::account::models::$domain::$variant => Self::$variant),+ } }
        }
        impl From<$stored> for crate::services::account::models::$domain {
            fn from(value: $stored) -> Self { match value { $($stored::$variant => Self::$variant),+ } }
        }
    };
}
map_enum!(
    StoredAccountStatus,
    AccountStatus,
    PendingVerification,
    Active,
    Suspended,
    Deleted
);
map_enum!(StoredIdentifierKind, IdentifierKind, Username, Email);
map_enum!(
    StoredChallengePurpose,
    ChallengePurpose,
    VerifyEmail,
    ResetPassword,
    ReplaceEmail
);
impl From<crate::services::authorization::models::SystemPermission> for SystemPermissionValue {
    fn from(value: crate::services::authorization::models::SystemPermission) -> Self {
        use crate::services::authorization::models::SystemPermission;
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
impl From<SystemPermissionValue> for crate::services::authorization::models::SystemPermission {
    fn from(value: SystemPermissionValue) -> Self {
        match value {
            SystemPermissionValue::AccountsRead => Self::AccountsRead,
            SystemPermissionValue::AccountsCreate => Self::AccountsCreate,
            SystemPermissionValue::AccountsSuspend => Self::AccountsSuspend,
            SystemPermissionValue::AccountsRestore => Self::AccountsRestore,
            SystemPermissionValue::AccountsDelete => Self::AccountsDelete,
            SystemPermissionValue::SettingsRead => Self::SettingsRead,
            SystemPermissionValue::SettingsUpdate => Self::SettingsUpdate,
        }
    }
}
