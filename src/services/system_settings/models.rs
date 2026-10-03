use super::{
    branding::PublicBranding,
    mail::MailSettings,
    policy::{ChallengeLifetime, EmailVerificationPolicy, RegistrationPolicy, SessionLifetime},
    site::PublicSiteUrls,
};
use crate::services::account::Result;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SystemSettings {
    pub revision: u64,
    pub branding: PublicBranding,
    pub site_urls: PublicSiteUrls,
    pub registration: RegistrationPolicy,
    pub email_verification: EmailVerificationPolicy,
    pub session_lifetime: SessionLifetime,
    pub challenge_lifetime: ChallengeLifetime,
    pub mail: MailSettings,
}

/// Non-secret authentication policy used on request paths.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthenticationSettings {
    pub revision: u64,
    pub site_urls: PublicSiteUrls,
    pub registration: RegistrationPolicy,
    pub email_verification: EmailVerificationPolicy,
    pub session_lifetime: SessionLifetime,
    pub challenge_lifetime: ChallengeLifetime,
    pub mail_enabled: bool,
}
impl SystemSettings {
    pub fn authentication(&self) -> AuthenticationSettings {
        AuthenticationSettings {
            revision: self.revision,
            site_urls: self.site_urls.clone(),
            registration: self.registration,
            email_verification: self.email_verification,
            session_lifetime: self.session_lifetime.clone(),
            challenge_lifetime: self.challenge_lifetime.clone(),
            mail_enabled: self.mail.configured(),
        }
    }
    pub fn validate(mut self) -> Result<Self> {
        self.mail = self.mail.validate()?;
        super::policy::validate_registration_delivery(
            self.registration,
            self.email_verification,
            self.mail.configured(),
        )?;
        Ok(self)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SetupState {
    NeedsSetup,
    Ready,
}
