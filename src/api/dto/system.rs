use super::account::RegisterRequest;
use crate::services::{
    account::Result,
    system_settings::{branding::*, mail::*, models::*, policy::*, site::PublicSiteUrls},
    system_setup::SetupCommand,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(utoipa::ToSchema))]
pub enum SetupStateDto {
    NeedsSetup,
    Ready,
}
impl From<SetupState> for SetupStateDto {
    fn from(value: SetupState) -> Self {
        match value {
            SetupState::NeedsSetup => Self::NeedsSetup,
            SetupState::Ready => Self::Ready,
        }
    }
}
crate::api::response::response!(SetupStatusResponse, SetupStateDto);

#[derive(Clone, Copy, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(utoipa::ToSchema))]
pub enum RegistrationDto {
    #[default]
    Closed,
    Open,
}
impl From<RegistrationDto> for RegistrationPolicy {
    fn from(v: RegistrationDto) -> Self {
        match v {
            RegistrationDto::Closed => Self::Closed,
            RegistrationDto::Open => Self::Open,
        }
    }
}
impl From<RegistrationPolicy> for RegistrationDto {
    fn from(v: RegistrationPolicy) -> Self {
        match v {
            RegistrationPolicy::Closed => Self::Closed,
            RegistrationPolicy::Open => Self::Open,
        }
    }
}

#[derive(Clone, Copy, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(utoipa::ToSchema))]
pub enum EmailVerificationDto {
    #[default]
    Optional,
    Required,
}
impl From<EmailVerificationDto> for EmailVerificationPolicy {
    fn from(v: EmailVerificationDto) -> Self {
        match v {
            EmailVerificationDto::Optional => Self::Optional,
            EmailVerificationDto::Required => Self::Required,
        }
    }
}
impl From<EmailVerificationPolicy> for EmailVerificationDto {
    fn from(v: EmailVerificationPolicy) -> Self {
        match v {
            EmailVerificationPolicy::Optional => Self::Optional,
            EmailVerificationPolicy::Required => Self::Required,
        }
    }
}

#[derive(Clone, Copy, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(utoipa::ToSchema))]
pub enum SmtpSecurityDto {
    #[default]
    StartTls,
    ImplicitTls,
    LoopbackPlaintext,
}
impl From<SmtpSecurityDto> for SmtpSecurity {
    fn from(v: SmtpSecurityDto) -> Self {
        match v {
            SmtpSecurityDto::StartTls => Self::StartTls,
            SmtpSecurityDto::ImplicitTls => Self::ImplicitTls,
            SmtpSecurityDto::LoopbackPlaintext => Self::LoopbackPlaintext,
        }
    }
}
impl From<SmtpSecurity> for SmtpSecurityDto {
    fn from(v: SmtpSecurity) -> Self {
        match v {
            SmtpSecurity::StartTls => Self::StartTls,
            SmtpSecurity::ImplicitTls => Self::ImplicitTls,
            SmtpSecurity::LoopbackPlaintext => Self::LoopbackPlaintext,
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(utoipa::ToSchema))]
pub struct MailSettingsInput {
    pub host: String,
    pub port: u16,
    #[serde(default)]
    pub username: String,
    pub password: Option<String>,
    pub from_address: String,
    #[serde(default = "brand")]
    pub from_name: String,
    #[serde(default)]
    pub security: SmtpSecurityDto,
}
fn brand() -> String {
    "AsterGate".into()
}
impl MailSettingsInput {
    pub fn command(self, previous: Option<&MailSettings>) -> Result<MailSettings> {
        MailSettings {
            host: self.host,
            port: self.port,
            username: self.username,
            password: match self.password {
                Some(value) => SecretString::new(value)?,
                None => previous.map(|m| m.password.clone()).unwrap_or_default(),
            },
            from_address: self.from_address,
            from_name: self.from_name,
            security: self.security.into(),
        }
        .validate()
    }
}
fn disabled_mail() -> MailSettings {
    MailSettings {
        host: String::new(),
        port: 587,
        username: String::new(),
        password: SecretString::default(),
        from_address: String::new(),
        from_name: brand(),
        security: SmtpSecurity::StartTls,
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(utoipa::ToSchema))]
pub struct SetupRequest {
    pub site_urls: Vec<String>,
    #[serde(default)]
    pub branding_title: String,
    #[serde(default)]
    pub branding_description: String,
    pub initial_account: RegisterRequest,
    #[serde(default)]
    pub registration: RegistrationDto,
    #[serde(default)]
    pub email_verification: EmailVerificationDto,
    pub mail: Option<MailSettingsInput>,
}
impl SetupRequest {
    pub fn command(self) -> Result<SetupCommand> {
        Ok(SetupCommand {
            settings: SystemSettings {
                revision: 0,
                branding: PublicBranding {
                    title: BrandingTitle::parse(&self.branding_title)?,
                    description: BrandingDescription::parse(&self.branding_description)?,
                },
                site_urls: PublicSiteUrls::parse(&self.site_urls)?,
                registration: self.registration.into(),
                email_verification: self.email_verification.into(),
                session_lifetime: SessionLifetime::new(86400)?,
                challenge_lifetime: ChallengeLifetime::new(1800)?,
                mail: self
                    .mail
                    .map(|m| m.command(None))
                    .transpose()?
                    .unwrap_or_else(disabled_mail),
            }
            .validate()?,
            initial_account: self.initial_account.try_into()?,
        })
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(utoipa::ToSchema))]
pub struct SettingsRequest {
    pub revision: u64,
    pub site_urls: Vec<String>,
    pub branding_title: String,
    pub branding_description: String,
    pub current_password: Option<String>,
    pub registration: RegistrationDto,
    pub email_verification: EmailVerificationDto,
    pub session_ttl_seconds: i64,
    pub challenge_ttl_seconds: i64,
    pub mail: Option<MailSettingsInput>,
}
impl SettingsRequest {
    pub fn command(self, previous: &SystemSettings) -> Result<SystemSettings> {
        SystemSettings {
            revision: self.revision,
            branding: PublicBranding {
                title: BrandingTitle::parse(&self.branding_title)?,
                description: BrandingDescription::parse(&self.branding_description)?,
            },
            site_urls: PublicSiteUrls::parse(&self.site_urls)?,
            registration: self.registration.into(),
            email_verification: self.email_verification.into(),
            session_lifetime: SessionLifetime::new(self.session_ttl_seconds)?,
            challenge_lifetime: ChallengeLifetime::new(self.challenge_ttl_seconds)?,
            mail: self
                .mail
                .map(|m| m.command(Some(&previous.mail)))
                .transpose()?
                .unwrap_or_else(disabled_mail),
        }
        .validate()
    }
}

#[derive(Serialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(utoipa::ToSchema))]
pub struct MailSettingsDto {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub has_password: bool,
    pub from_address: String,
    pub from_name: String,
    pub security: SmtpSecurityDto,
}
#[derive(Serialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(utoipa::ToSchema))]
pub struct SettingsDto {
    pub revision: u64,
    pub branding_title: String,
    pub branding_description: String,
    pub site_urls: Vec<String>,
    pub registration: RegistrationDto,
    pub email_verification: EmailVerificationDto,
    pub session_ttl_seconds: i64,
    pub challenge_ttl_seconds: i64,
    pub mail: Option<MailSettingsDto>,
}
impl From<SystemSettings> for SettingsDto {
    fn from(v: SystemSettings) -> Self {
        Self {
            revision: v.revision,
            branding_title: v.branding.title.as_str().into(),
            branding_description: v.branding.description.as_str().into(),
            site_urls: v.site_urls.values(),
            registration: v.registration.into(),
            email_verification: v.email_verification.into(),
            session_ttl_seconds: v.session_lifetime.seconds(),
            challenge_ttl_seconds: v.challenge_lifetime.seconds(),
            mail: v.mail.configured().then(|| MailSettingsDto {
                host: v.mail.host,
                port: v.mail.port,
                username: v.mail.username,
                has_password: !v.mail.password.is_empty(),
                from_address: v.mail.from_address,
                from_name: v.mail.from_name,
                security: v.mail.security.into(),
            }),
        }
    }
}
crate::api::response::response!(SettingsResponse, SettingsDto);
