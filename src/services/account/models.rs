use super::{AccountError, Result};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AccountId(pub(crate) String);

impl AccountId {
    pub fn parse(value: &str) -> Result<Self> {
        let id = uuid::Uuid::parse_str(value).map_err(|_| AccountError::InvalidInput)?;
        Ok(Self(id.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
    pub(crate) fn new() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionId(pub(crate) String);

impl SessionId {
    pub fn parse(value: &str) -> Result<Self> {
        Ok(Self(AccountId::parse(value)?.0))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AccountStatus {
    PendingVerification,
    Active,
    Suspended,
    Deleted,
}

impl AccountStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::PendingVerification => "pending_verification",
            Self::Active => "active",
            Self::Suspended => "suspended",
            Self::Deleted => "deleted",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IdentifierKind {
    Username,
    Email,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChallengePurpose {
    VerifyEmail,
    ResetPassword,
    ReplaceEmail,
}

impl ChallengePurpose {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::VerifyEmail => "verify_email",
            Self::ResetPassword => "reset_password",
            Self::ReplaceEmail => "replace_email",
        }
    }
}

pub struct RegisterAccount {
    pub username: Username,
    pub email: EmailAddress,
    pub display_name: DisplayName,
    pub password: NewPassword,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Username(String);
impl Username {
    pub fn parse(value: &str) -> Result<Self> {
        let value = value.trim().to_ascii_lowercase();
        if !(3..=32).contains(&value.len())
            || !value
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
        {
            return Err(AccountError::InvalidInput);
        }
        Ok(Self(value))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EmailAddress(String);
impl EmailAddress {
    pub fn parse(value: &str) -> Result<Self> {
        aster_forge_validation::email::normalize_email(value)
            .map(Self)
            .map_err(|_| AccountError::InvalidInput)
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DisplayName(String);
impl DisplayName {
    pub fn parse(value: &str) -> Result<Self> {
        let value = value.trim();
        if value.is_empty() || value.len() > 128 || value.chars().any(char::is_control) {
            return Err(AccountError::InvalidInput);
        }
        Ok(Self(value.into()))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
pub struct NewPassword(String);
impl NewPassword {
    pub fn parse(value: String) -> Result<Self> {
        if !(12..=128).contains(&value.len()) {
            return Err(AccountError::InvalidInput);
        }
        Ok(Self(value))
    }
    pub(crate) fn into_secret(self) -> String {
        self.0
    }
}
impl std::fmt::Debug for NewPassword {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("[REDACTED]")
    }
}
impl RegisterAccount {
    pub fn parse(
        username: &str,
        email: &str,
        display_name: &str,
        password: String,
    ) -> Result<Self> {
        Ok(Self {
            username: Username::parse(username)?,
            email: EmailAddress::parse(email)?,
            display_name: DisplayName::parse(display_name)?,
            password: NewPassword::parse(password)?,
        })
    }
}

pub struct Account {
    pub id: AccountId,
    pub username: String,
    pub email: String,
    pub email_verified: bool,
    pub display_name: String,
    pub status: AccountStatus,
    pub created_at: i64,
    pub updated_at: i64,
    pub system_permissions: Vec<crate::services::authorization::models::SystemPermission>,
}

pub struct Principal {
    pub(crate) account_id: AccountId,
    pub(crate) session_id: SessionId,
    pub(crate) token_hash: String,
    pub(crate) revision: i64,
}

pub struct LoginResult {
    pub account: Account,
    pub session_token: String,
}

pub struct Session {
    pub id: SessionId,
    pub created_at: i64,
    pub expires_at: i64,
    pub user_agent: String,
    pub current: bool,
}

#[derive(Clone, Copy)]
pub enum LifecycleAction {
    Suspend,
    Restore,
    Delete,
}

#[derive(Clone, Copy)]
pub enum AuditAction {
    Initialized,
    SettingsUpdated,
    Registered,
    SignedIn,
    SignedOut,
    ProfileUpdated,
    PasswordChanged,
    PasswordReset,
    EmailVerified,
    EmailChanged,
    ChallengeRequested,
    SessionRevoked,
    SessionsRevoked,
    Suspended,
    Restored,
    Deleted,
    Provisioned,
}

#[derive(Clone, Copy)]
pub enum AuthenticationOperation {
    Register,
    RegisterGlobal,
    Login,
    LoginGlobal,
    LoginAccount,
    StepUp,
    VerifyEmail,
    ResetPassword,
    ReplaceEmail,
    MailGlobal,
    ResetConsume,
}
impl AuthenticationOperation {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Register => "register",
            Self::RegisterGlobal => "register_global",
            Self::Login => "login",
            Self::LoginGlobal => "login_global",
            Self::LoginAccount => "login_account",
            Self::StepUp => "step_up",
            Self::VerifyEmail => "verify_email",
            Self::ResetPassword => "reset_password",
            Self::ReplaceEmail => "replace_email",
            Self::MailGlobal => "mail_global",
            Self::ResetConsume => "reset_consume",
        }
    }
}
impl From<ChallengePurpose> for AuthenticationOperation {
    fn from(value: ChallengePurpose) -> Self {
        match value {
            ChallengePurpose::VerifyEmail => Self::VerifyEmail,
            ChallengePurpose::ResetPassword => Self::ResetPassword,
            ChallengePurpose::ReplaceEmail => Self::ReplaceEmail,
        }
    }
}

impl AuditAction {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Initialized => "instance.initialized",
            Self::SettingsUpdated => "instance.settings_updated",
            Self::Registered => "account.registered",
            Self::SignedIn => "account.signed_in",
            Self::SignedOut => "account.signed_out",
            Self::ProfileUpdated => "account.profile_updated",
            Self::PasswordChanged => "account.password_changed",
            Self::PasswordReset => "account.password_reset",
            Self::EmailVerified => "account.email_verified",
            Self::EmailChanged => "account.email_changed",
            Self::ChallengeRequested => "account.challenge_requested",
            Self::SessionRevoked => "account.session_revoked",
            Self::SessionsRevoked => "account.sessions_revoked",
            Self::Suspended => "account.suspended",
            Self::Restored => "account.restored",
            Self::Deleted => "account.deleted",
            Self::Provisioned => "account.provisioned",
        }
    }
}
