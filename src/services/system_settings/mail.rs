use crate::services::account::{AccountError, Result};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SmtpSecurity {
    StartTls,
    ImplicitTls,
    LoopbackPlaintext,
}
impl SmtpSecurity {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::StartTls => "start_tls",
            Self::ImplicitTls => "implicit_tls",
            Self::LoopbackPlaintext => "loopback_plaintext",
        }
    }
    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "start_tls" => Ok(Self::StartTls),
            "implicit_tls" => Ok(Self::ImplicitTls),
            "loopback_plaintext" => Ok(Self::LoopbackPlaintext),
            _ => Err(AccountError::InvalidInput),
        }
    }
}

#[derive(Clone, Default, PartialEq, Eq)]
pub struct SecretString(String);
impl SecretString {
    pub fn new(value: String) -> Result<Self> {
        if value.len() <= 4096 && !value.contains(['\r', '\n']) {
            Ok(Self(value))
        } else {
            Err(AccountError::InvalidInput)
        }
    }
    pub(crate) fn expose(&self) -> &str {
        &self.0
    }
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}
impl std::fmt::Debug for SecretString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("[REDACTED]")
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MailSettings {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: SecretString,
    pub from_address: String,
    pub from_name: String,
    pub security: SmtpSecurity,
}
impl MailSettings {
    pub fn validate(mut self) -> Result<Self> {
        self.host = normalize_smtp_host(&self.host)?;
        self.username = normalize_smtp_username(&self.username)?;
        self.from_name = normalize_sender_name(&self.from_name)?;
        if self.port == 0
            || self.host.len() > 253
            || self.host.chars().any(char::is_whitespace)
            || self.username.len() > 254
            || self.username.chars().any(char::is_control)
            || self.from_name.len() > 128
            || self.from_name.chars().any(char::is_control)
            || (!self.host.is_empty()
                && self.security == SmtpSecurity::LoopbackPlaintext
                && !matches!(self.host.as_str(), "localhost" | "127.0.0.1" | "::1"))
            || (self.security == SmtpSecurity::ImplicitTls && self.port != 465)
            || (self.security == SmtpSecurity::StartTls && self.port == 465)
        {
            return Err(AccountError::InvalidInput);
        }
        if !self.host.is_empty() {
            self.from_address = aster_forge_validation::email::normalize_email(&self.from_address)
                .map_err(|_| AccountError::InvalidInput)?;
            if self.username.is_empty() != self.password.is_empty() {
                return Err(AccountError::InvalidInput);
            }
        } else if !self.username.is_empty()
            || !self.password.is_empty()
            || !self.from_address.is_empty()
        {
            return Err(AccountError::InvalidInput);
        }
        Ok(self)
    }
    pub fn configured(&self) -> bool {
        !self.host.is_empty()
    }
    pub fn runtime(&self) -> aster_forge_mail::MailRuntimeSettings {
        aster_forge_mail::MailRuntimeSettings {
            smtp_host: self.host.clone(),
            smtp_port: self.port,
            smtp_username: self.username.clone(),
            smtp_password: self.password.expose().into(),
            from_address: self.from_address.clone(),
            from_name: self.from_name.clone(),
            encryption_enabled: self.security != SmtpSecurity::LoopbackPlaintext,
        }
    }
}

pub fn normalize_smtp_host(value: &str) -> Result<String> {
    let value = value.trim().to_ascii_lowercase();
    if value.len() > 253
        || value.chars().any(|c| c.is_control() || c.is_whitespace())
        || value.contains(['/', '\\', '@', '*'])
    {
        return Err(AccountError::InvalidInput);
    }
    Ok(value)
}
pub fn normalize_smtp_username(value: &str) -> Result<String> {
    if value.len() > 254 || value.chars().any(char::is_control) {
        return Err(AccountError::InvalidInput);
    }
    Ok(value.into())
}
pub fn normalize_sender_name(value: &str) -> Result<String> {
    aster_forge_validation::display::normalize_bounded_display_text("mail_from_name", value, 128)
        .map_err(|_| AccountError::InvalidInput)
}
