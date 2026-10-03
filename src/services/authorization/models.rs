//! Instance administration permissions. Tenant and application authorization have separate owners.

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SystemPermission {
    AccountsRead,
    AccountsCreate,
    AccountsSuspend,
    AccountsRestore,
    AccountsDelete,
    SettingsRead,
    SettingsUpdate,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SystemResource {
    Accounts,
    Settings,
}
impl SystemResource {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Accounts => "accounts",
            Self::Settings => "settings",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PermissionAction {
    Read,
    Create,
    Suspend,
    Restore,
    Delete,
    Update,
}
impl PermissionAction {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Read => "read",
            Self::Create => "create",
            Self::Suspend => "suspend",
            Self::Restore => "restore",
            Self::Delete => "delete",
            Self::Update => "update",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuthenticationRequirement {
    Session,
    CurrentPassword,
}

#[derive(Clone, Copy, Debug)]
pub struct SystemPermissionDefinition {
    pub permission: SystemPermission,
    pub resource: SystemResource,
    pub action: PermissionAction,
    pub authentication: AuthenticationRequirement,
    pub protects_last_holder: bool,
}

impl SystemPermission {
    pub const ALL: [Self; 7] = [
        Self::AccountsRead,
        Self::AccountsCreate,
        Self::AccountsSuspend,
        Self::AccountsRestore,
        Self::AccountsDelete,
        Self::SettingsRead,
        Self::SettingsUpdate,
    ];
    pub const NAMESPACE: &'static str = "astergate.system";
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AccountsRead => "astergate.system.accounts.read",
            Self::AccountsCreate => "astergate.system.accounts.create",
            Self::AccountsSuspend => "astergate.system.accounts.suspend",
            Self::AccountsRestore => "astergate.system.accounts.restore",
            Self::AccountsDelete => "astergate.system.accounts.delete",
            Self::SettingsRead => "astergate.system.settings.read",
            Self::SettingsUpdate => "astergate.system.settings.update",
        }
    }
    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|permission| permission.as_str() == value)
    }
    pub const fn definition(self) -> SystemPermissionDefinition {
        use AuthenticationRequirement::{CurrentPassword, Session};
        use PermissionAction::{Create, Delete, Read, Restore, Suspend, Update};
        use SystemResource::{Accounts, Settings};
        let (resource, action, authentication) = match self {
            Self::AccountsRead => (Accounts, Read, Session),
            Self::AccountsCreate => (Accounts, Create, Session),
            Self::AccountsSuspend => (Accounts, Suspend, CurrentPassword),
            Self::AccountsRestore => (Accounts, Restore, CurrentPassword),
            Self::AccountsDelete => (Accounts, Delete, CurrentPassword),
            Self::SettingsRead => (Settings, Read, Session),
            Self::SettingsUpdate => (Settings, Update, CurrentPassword),
        };
        SystemPermissionDefinition {
            permission: self,
            resource,
            action,
            authentication,
            protects_last_holder: matches!(self, Self::AccountsCreate | Self::SettingsUpdate),
        }
    }
}

/// Validated logical key for future product-defined permissions; it does not confer authorization.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct PermissionKey(String);
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("invalid permission key")]
pub struct InvalidPermissionKey;

impl PermissionKey {
    pub const MAX_LEN: usize = 128;
    pub fn parse(value: &str) -> Result<Self, InvalidPermissionKey> {
        if value.len() > Self::MAX_LEN || value.is_empty() {
            return Err(InvalidPermissionKey);
        }
        let mut segments = value.split('.');
        let valid = |segment: &str| {
            !segment.is_empty()
                && segment.as_bytes()[0].is_ascii_lowercase()
                && segment
                    .bytes()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_')
        };
        let first = segments.next().ok_or(InvalidPermissionKey)?;
        if !valid(first) || segments.clone().count() < 1 || !segments.all(valid) {
            return Err(InvalidPermissionKey);
        }
        // The entire astergate namespace is reserved for explicitly registered product permissions.
        if first == "astergate" && SystemPermission::parse(value).is_none() {
            return Err(InvalidPermissionKey);
        }
        Ok(Self(value.into()))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<SystemPermission> for PermissionKey {
    fn from(value: SystemPermission) -> Self {
        Self(value.as_str().into())
    }
}
