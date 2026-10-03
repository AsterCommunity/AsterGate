//! Registry adapters delegate to the same domain constructors used by service commands.
use crate::services::system_settings::{
    branding::*,
    mail::{SmtpSecurity, normalize_sender_name, normalize_smtp_host, normalize_smtp_username},
    policy::*,
    site::PublicSiteUrls,
};
use aster_forge_config::{ConfigCoreError, ConfigValueLookup, Result};

fn invalid() -> ConfigCoreError {
    ConfigCoreError::invalid_value("invalid system configuration value")
}

pub fn registration_delivery(lookup: &dyn ConfigValueLookup, key: &str, value: &str) -> Result<()> {
    use super::definitions::{
        ACCOUNT_EMAIL_VERIFICATION_KEY, ACCOUNT_REGISTRATION_KEY, MAIL_SMTP_HOST_KEY,
    };
    let effective = |candidate_key| {
        if candidate_key == key {
            Some(value.to_string())
        } else {
            lookup.get_config_value(candidate_key)
        }
    };
    let registration = RegistrationPolicy::parse(
        effective(ACCOUNT_REGISTRATION_KEY)
            .as_deref()
            .unwrap_or("closed"),
    )
    .map_err(|_| invalid())?;
    let verification = EmailVerificationPolicy::parse(
        effective(ACCOUNT_EMAIL_VERIFICATION_KEY)
            .as_deref()
            .unwrap_or("optional"),
    )
    .map_err(|_| invalid())?;
    let mail_enabled = effective(MAIL_SMTP_HOST_KEY).is_some_and(|host| !host.trim().is_empty());
    validate_registration_delivery(registration, verification, mail_enabled).map_err(|_| invalid())
}

macro_rules! text_normalizer {
    ($name:ident, $type:ident) => {
        pub fn $name(_: &dyn ConfigValueLookup, _: &str, value: &str) -> Result<String> {
            $type::parse(value)
                .map(|value| value.as_str().into())
                .map_err(|_| invalid())
        }
    };
}
text_normalizer!(branding_title, BrandingTitle);
text_normalizer!(branding_description, BrandingDescription);
text_normalizer!(registration, RegistrationPolicy);
text_normalizer!(email_verification, EmailVerificationPolicy);
text_normalizer!(smtp_security, SmtpSecurity);

pub fn site_urls(lookup: &dyn ConfigValueLookup, _: &str, value: &str) -> Result<String> {
    let values: Vec<String> = serde_json::from_str(value).map_err(|_| invalid())?;
    if values.is_empty()
        && lookup
            .get_config_value(super::definitions::SETUP_COMPLETED_KEY)
            .as_deref()
            != Some("true")
    {
        return Ok("[]".into());
    }
    let origins = PublicSiteUrls::parse(&values).map_err(|_| invalid())?;
    serde_json::to_string(&origins.values()).map_err(|_| invalid())
}
pub fn session_lifetime(_: &dyn ConfigValueLookup, _: &str, value: &str) -> Result<String> {
    SessionLifetime::new(value.parse().map_err(|_| invalid())?)
        .map(|value| value.seconds().to_string())
        .map_err(|_| invalid())
}
pub fn challenge_lifetime(_: &dyn ConfigValueLookup, _: &str, value: &str) -> Result<String> {
    ChallengeLifetime::new(value.parse().map_err(|_| invalid())?)
        .map(|value| value.seconds().to_string())
        .map_err(|_| invalid())
}
pub fn revision(_: &dyn ConfigValueLookup, _: &str, value: &str) -> Result<String> {
    value
        .parse::<u64>()
        .map(|value| value.to_string())
        .map_err(|_| invalid())
}
pub fn smtp_host(_: &dyn ConfigValueLookup, _: &str, value: &str) -> Result<String> {
    normalize_smtp_host(value).map_err(|_| invalid())
}
pub fn smtp_username(_: &dyn ConfigValueLookup, _: &str, value: &str) -> Result<String> {
    normalize_smtp_username(value).map_err(|_| invalid())
}
pub fn sender_name(_: &dyn ConfigValueLookup, _: &str, value: &str) -> Result<String> {
    normalize_sender_name(value).map_err(|_| invalid())
}
pub fn sender_address(_: &dyn ConfigValueLookup, _: &str, value: &str) -> Result<String> {
    if value.is_empty() {
        return Ok(String::new());
    }
    aster_forge_validation::email::normalize_email(value).map_err(|_| invalid())
}
pub fn smtp_port(_: &dyn ConfigValueLookup, _: &str, value: &str) -> Result<String> {
    let port: u16 = value.parse().map_err(|_| invalid())?;
    if port == 0 {
        return Err(invalid());
    }
    Ok(port.to_string())
}
