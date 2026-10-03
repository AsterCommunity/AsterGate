pub(crate) mod challenges;
mod lifecycle;
pub mod models;
mod security;
pub use challenges::*;
pub use lifecycle::*;
pub(crate) use lifecycle::{
    audit as write_audit, project as project_account, prove_password as verify_current_password,
};
pub use security::AccountSecurity;
pub(crate) use security::{digest as token_digest, random_token as new_token};

pub type Result<T> = std::result::Result<T, AccountError>;

#[derive(Clone, Copy, Debug, thiserror::Error)]
pub enum AccountError {
    #[error("invalid input")]
    InvalidInput,
    #[error("invalid credentials")]
    InvalidCredentials,
    #[error("authentication required")]
    Unauthenticated,
    #[error("access denied")]
    Forbidden,
    #[error("account or identifier conflict")]
    Conflict,
    #[error("settings changed; reload before saving")]
    SettingsConflict,
    #[error("resource not found")]
    NotFound,
    #[error("invalid lifecycle transition")]
    InvalidTransition,
    #[error("challenge invalid or expired")]
    InvalidChallenge,
    #[error("too many attempts")]
    RateLimited,
    #[error("mail is not configured")]
    MailUnavailable,
    #[error("storage unavailable")]
    Storage,
    #[error("security operation unavailable")]
    Security,
    #[error("system setup required")]
    SetupRequired,
    #[error("system already initialized")]
    AlreadyInitialized,
    #[error("last active instance operator cannot be removed")]
    LastOperator,
}

impl From<sea_orm::DbErr> for AccountError {
    fn from(error: sea_orm::DbErr) -> Self {
        if matches!(
            error.sql_err(),
            Some(sea_orm::SqlErr::UniqueConstraintViolation(_))
        ) {
            Self::Conflict
        } else {
            tracing::error!("account persistence failed");
            Self::Storage
        }
    }
}

impl From<aster_forge_crypto::CryptoError> for AccountError {
    fn from(_: aster_forge_crypto::CryptoError) -> Self {
        Self::Security
    }
}

pub(crate) fn now() -> i64 {
    chrono::Utc::now().timestamp()
}
