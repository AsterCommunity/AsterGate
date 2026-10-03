use crate::services::account::{AccountError, Result};

pub fn validate_registration_delivery(
    registration: RegistrationPolicy,
    verification: EmailVerificationPolicy,
    mail_enabled: bool,
) -> Result<()> {
    if registration == RegistrationPolicy::Open
        && verification == EmailVerificationPolicy::Required
        && !mail_enabled
    {
        Err(AccountError::MailUnavailable)
    } else {
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionLifetime(i64);
impl SessionLifetime {
    pub fn new(value: i64) -> Result<Self> {
        if (300..=2_592_000).contains(&value) {
            Ok(Self(value))
        } else {
            Err(AccountError::InvalidInput)
        }
    }
    pub fn seconds(&self) -> i64 {
        self.0
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChallengeLifetime(i64);
impl ChallengeLifetime {
    pub fn new(value: i64) -> Result<Self> {
        if (60..=86400).contains(&value) {
            Ok(Self(value))
        } else {
            Err(AccountError::InvalidInput)
        }
    }
    pub fn seconds(&self) -> i64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RegistrationPolicy {
    Closed,
    Open,
}
impl RegistrationPolicy {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Closed => "closed",
            Self::Open => "open",
        }
    }
    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "closed" => Ok(Self::Closed),
            "open" => Ok(Self::Open),
            _ => Err(AccountError::InvalidInput),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EmailVerificationPolicy {
    Optional,
    Required,
}
impl EmailVerificationPolicy {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Optional => "optional",
            Self::Required => "required",
        }
    }
    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "optional" => Ok(Self::Optional),
            "required" => Ok(Self::Required),
            _ => Err(AccountError::InvalidInput),
        }
    }
}
