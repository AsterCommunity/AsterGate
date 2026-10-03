//! Runtime configuration keys and registry metadata, following AsterDrive's definitions contract.
use crate::services::system_settings::branding::{
    DEFAULT_BRANDING_DESCRIPTION, DEFAULT_BRANDING_TITLE,
};
use aster_forge_config::{ConfigDefinition, ConfigValueType, ConfigVisibility};

pub const BRANDING_TITLE_KEY: &str = "branding_title";
pub const BRANDING_DESCRIPTION_KEY: &str = "branding_description";
pub const PUBLIC_SITE_URLS_KEY: &str = "public_site_urls";
pub const ACCOUNT_REGISTRATION_KEY: &str = "account_registration";
pub const ACCOUNT_EMAIL_VERIFICATION_KEY: &str = "account_email_verification";
pub const ACCOUNT_SESSION_TTL_KEY: &str = "account_session_ttl";
pub const ACCOUNT_CHALLENGE_TTL_KEY: &str = "account_challenge_ttl";
pub const MAIL_SMTP_HOST_KEY: &str = "mail_smtp_host";
pub const MAIL_SMTP_PORT_KEY: &str = "mail_smtp_port";
pub const MAIL_SMTP_USERNAME_KEY: &str = "mail_smtp_username";
pub const MAIL_SMTP_PASSWORD_KEY: &str = "mail_smtp_password";
pub const MAIL_FROM_ADDRESS_KEY: &str = "mail_from_address";
pub const MAIL_FROM_NAME_KEY: &str = "mail_from_name";
pub const MAIL_SMTP_SECURITY_KEY: &str = "mail_smtp_security";
pub const SETUP_COMPLETED_KEY: &str = "setup_completed";
pub const SETTINGS_REVISION_KEY: &str = "settings_revision";

pub static CONFIG_DEFINITIONS: [ConfigDefinition; 16] = [
    ConfigDefinition {
        key: BRANDING_TITLE_KEY,
        normalize_fn: Some(super::normalizers::branding_title),
        label_i18n_key: "siteName",
        value_type: ConfigValueType::String,
        default_fn: || DEFAULT_BRANDING_TITLE.into(),
        category: "site",
        description: "Public site name, up to 120 UTF-8 bytes",
        visibility: ConfigVisibility::Public,
        ..ConfigDefinition::private_system()
    },
    ConfigDefinition {
        key: BRANDING_DESCRIPTION_KEY,
        normalize_fn: Some(super::normalizers::branding_description),
        label_i18n_key: "siteDescription",
        value_type: ConfigValueType::String,
        default_fn: || DEFAULT_BRANDING_DESCRIPTION.into(),
        category: "site",
        description: "Public description, up to 300 UTF-8 bytes",
        visibility: ConfigVisibility::Public,
        ..ConfigDefinition::private_system()
    },
    ConfigDefinition {
        key: PUBLIC_SITE_URLS_KEY,
        normalize_fn: Some(super::normalizers::site_urls),
        label_i18n_key: "publicSites",
        value_type: ConfigValueType::StringArray,
        default_fn: || "[]".into(),
        category: "site",
        description: "Ordered trusted frontend origins; first is primary for mail; independent of CORS",

        ..ConfigDefinition::private_system()
    },
    ConfigDefinition {
        key: ACCOUNT_REGISTRATION_KEY,
        dependency_validator_fn: Some(super::normalizers::registration_delivery),
        normalize_fn: Some(super::normalizers::registration),
        label_i18n_key: "allowRegistration",
        value_type: ConfigValueType::String,
        default_fn: || "closed".into(),
        category: "account",
        description: "Open or closed account registration",

        ..ConfigDefinition::private_system()
    },
    ConfigDefinition {
        key: ACCOUNT_EMAIL_VERIFICATION_KEY,
        dependency_validator_fn: Some(super::normalizers::registration_delivery),
        normalize_fn: Some(super::normalizers::email_verification),
        label_i18n_key: "requireVerification",
        value_type: ConfigValueType::String,
        default_fn: || "optional".into(),
        category: "account",
        description: "Optional or required email verification on registration",

        ..ConfigDefinition::private_system()
    },
    ConfigDefinition {
        key: ACCOUNT_SESSION_TTL_KEY,
        normalize_fn: Some(super::normalizers::session_lifetime),
        label_i18n_key: "sessionLifetime",
        value_type: ConfigValueType::Number,
        default_fn: || "86400".into(),
        category: "account",
        description: "New session lifetime in seconds, 300 to 2592000",

        ..ConfigDefinition::private_system()
    },
    ConfigDefinition {
        key: ACCOUNT_CHALLENGE_TTL_KEY,
        normalize_fn: Some(super::normalizers::challenge_lifetime),
        label_i18n_key: "challengeLifetime",
        value_type: ConfigValueType::Number,
        default_fn: || "1800".into(),
        category: "account",
        description: "New mail challenge lifetime in seconds, 60 to 86400",

        ..ConfigDefinition::private_system()
    },
    ConfigDefinition {
        key: MAIL_SMTP_HOST_KEY,
        dependency_validator_fn: Some(super::normalizers::registration_delivery),
        normalize_fn: Some(super::normalizers::smtp_host),
        label_i18n_key: "smtpHost",
        value_type: ConfigValueType::String,
        default_fn: String::new,
        category: "mail",
        description: "Optional SMTP host; empty disables email delivery",

        ..ConfigDefinition::private_system()
    },
    ConfigDefinition {
        key: MAIL_SMTP_PORT_KEY,
        normalize_fn: Some(super::normalizers::smtp_port),
        label_i18n_key: "smtpPort",
        value_type: ConfigValueType::Number,
        default_fn: || "587".into(),
        category: "mail",
        description: "SMTP transport port",

        ..ConfigDefinition::private_system()
    },
    ConfigDefinition {
        key: MAIL_SMTP_USERNAME_KEY,
        normalize_fn: Some(super::normalizers::smtp_username),
        label_i18n_key: "smtpUsername",
        value_type: ConfigValueType::String,
        default_fn: String::new,
        category: "mail",
        description: "Optional SMTP authentication username",
        is_sensitive: true,
        ..ConfigDefinition::private_system()
    },
    ConfigDefinition {
        key: MAIL_SMTP_PASSWORD_KEY,
        label_i18n_key: "smtpPassword",
        value_type: ConfigValueType::String,
        default_fn: String::new,
        category: "mail",
        description: "Authenticated encrypted SMTP password; never exposed to clients",
        is_sensitive: true,
        ..ConfigDefinition::private_system()
    },
    ConfigDefinition {
        key: MAIL_FROM_ADDRESS_KEY,
        normalize_fn: Some(super::normalizers::sender_address),
        label_i18n_key: "senderEmail",
        value_type: ConfigValueType::String,
        default_fn: String::new,
        category: "mail",
        description: "Account email sender address",

        ..ConfigDefinition::private_system()
    },
    ConfigDefinition {
        key: MAIL_FROM_NAME_KEY,
        normalize_fn: Some(super::normalizers::sender_name),
        label_i18n_key: "senderName",
        value_type: ConfigValueType::String,
        default_fn: || "AsterGate".into(),
        category: "mail",
        description: "Account email sender display name",

        ..ConfigDefinition::private_system()
    },
    ConfigDefinition {
        key: MAIL_SMTP_SECURITY_KEY,
        normalize_fn: Some(super::normalizers::smtp_security),
        label_i18n_key: "transportSecurity",
        value_type: ConfigValueType::String,
        default_fn: || "start_tls".into(),
        category: "mail",
        description: "SMTP policy: start_tls, implicit_tls or loopback_plaintext",

        ..ConfigDefinition::private_system()
    },
    ConfigDefinition {
        key: SETUP_COMPLETED_KEY,
        value_type: ConfigValueType::Boolean,
        default_fn: || "false".into(),
        category: "setup",
        description: "Initial setup completed; managed only by setup transaction",

        ..ConfigDefinition::private_system()
    },
    ConfigDefinition {
        key: SETTINGS_REVISION_KEY,
        normalize_fn: Some(super::normalizers::revision),
        value_type: ConfigValueType::Number,
        default_fn: || "0".into(),
        category: "setup",
        description: "Settings revision; incremented only by successful settings transaction",

        ..ConfigDefinition::private_system()
    },
];
