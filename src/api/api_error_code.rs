//! Stable product error codes, shared with clients through OpenAPI.

macro_rules! codes {
    ($($variant:ident => $wire:literal, $message:literal;)+) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
        #[cfg_attr(all(debug_assertions, feature = "openapi"), derive(utoipa::ToSchema))]
        pub enum ApiErrorCode { $(#[serde(rename = $wire)] #[cfg_attr(all(debug_assertions, feature = "openapi"), schema(rename = $wire))] $variant,)+ }
        impl ApiErrorCode {
            pub const ALL: &'static [Self] = &[$(Self::$variant,)+];
            pub const fn as_str(self) -> &'static str { match self { $(Self::$variant => $wire,)+ } }
            pub fn parse(value: &str) -> Option<Self> { match value { $($wire => Some(Self::$variant),)+ _ => None } }
            pub const fn message(self) -> &'static str { match self { $(Self::$variant => $message,)+ } }
        }
    };
}

codes! {
    Success => "success", "";
    ValidationFailed => "validation.failed", "The request contains invalid values.";
    CredentialsFailed => "auth.credentials_failed", "Invalid credentials or inactive account.";
    SessionRequired => "auth.session_required", "An active account session is required.";
    Forbidden => "auth.forbidden", "This action is not permitted.";
    IdentifierConflict => "account.identifier_conflict", "Account identifiers or revision conflict.";
    SettingsConflict => "settings.revision_conflict", "Settings changed. Reload before saving.";
    NotFound => "account.not_found", "The requested resource was not found.";
    InvalidTransition => "account.invalid_transition", "The account state does not allow this action.";
    ChallengeInvalid => "auth.challenge_invalid", "The email link is invalid, expired or already used.";
    RateLimited => "rate_limited", "Too many attempts. Try again later.";
    MailNotConfigured => "mail.not_configured", "Email delivery is not configured.";
    ServiceUnavailable => "service.unavailable", "The service is temporarily unavailable.";
    SetupRequired => "setup.required", "Complete instance setup first.";
    AlreadyInitialized => "setup.already_initialized", "The instance has already been initialized.";
    LastOperator => "account.last_operator", "The last active instance operator cannot be removed.";
    EndpointNotFound => "endpoint.not_found", "The API endpoint was not found.";
}

impl From<&crate::services::account::AccountError> for ApiErrorCode {
    fn from(value: &crate::services::account::AccountError) -> Self {
        use crate::services::account::AccountError;
        match value {
            AccountError::InvalidInput => Self::ValidationFailed,
            AccountError::InvalidCredentials => Self::CredentialsFailed,
            AccountError::Unauthenticated => Self::SessionRequired,
            AccountError::Forbidden => Self::Forbidden,
            AccountError::Conflict => Self::IdentifierConflict,
            AccountError::SettingsConflict => Self::SettingsConflict,
            AccountError::NotFound => Self::NotFound,
            AccountError::InvalidTransition => Self::InvalidTransition,
            AccountError::InvalidChallenge => Self::ChallengeInvalid,
            AccountError::RateLimited => Self::RateLimited,
            AccountError::MailUnavailable => Self::MailNotConfigured,
            AccountError::Storage | AccountError::Security => Self::ServiceUnavailable,
            AccountError::SetupRequired => Self::SetupRequired,
            AccountError::AlreadyInitialized => Self::AlreadyInitialized,
            AccountError::LastOperator => Self::LastOperator,
        }
    }
}

impl ApiErrorCode {
    pub fn status(self) -> axum::http::StatusCode {
        use axum::http::StatusCode;
        match self {
            Self::Success => StatusCode::OK,
            Self::ValidationFailed | Self::ChallengeInvalid => StatusCode::BAD_REQUEST,
            Self::CredentialsFailed | Self::SessionRequired => StatusCode::UNAUTHORIZED,
            Self::Forbidden => StatusCode::FORBIDDEN,
            Self::IdentifierConflict
            | Self::SettingsConflict
            | Self::InvalidTransition
            | Self::SetupRequired
            | Self::AlreadyInitialized
            | Self::LastOperator => StatusCode::CONFLICT,
            Self::NotFound | Self::EndpointNotFound => StatusCode::NOT_FOUND,
            Self::RateLimited => StatusCode::TOO_MANY_REQUESTS,
            Self::MailNotConfigured | Self::ServiceUnavailable => StatusCode::SERVICE_UNAVAILABLE,
        }
    }
}
