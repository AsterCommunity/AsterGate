use aster_gate::{
    config::{AppConfig, SecurityConfig, security},
    services::system_settings::{branding::*, mail::*, models::*, policy::*, site::*},
};

#[test]
fn validates_typed_origin_lifetimes_and_optional_smtp() {
    for value in [
        "https://example.test/path",
        "https://example.test/?tenant=a",
        "https://example.test/#secret",
        "https://user:secret@example.test",
        "http://example.test",
        "file:///tmp/config",
    ] {
        assert!(PublicSiteOrigin::parse(value).is_err());
    }
    assert!(
        PublicSiteOrigin::parse("https://identity.example.test")
            .unwrap()
            .secure_cookies()
    );
    assert!(
        !PublicSiteOrigin::parse("http://127.0.0.1:4173")
            .unwrap()
            .secure_cookies()
    );
    assert!(SessionLifetime::new(0).is_err());
    assert!(ChallengeLifetime::new(i64::MAX).is_err());
    let settings = SystemSettings {
        revision: 1,
        branding: PublicBranding::default(),
        site_urls: PublicSiteUrls::parse(&["https://identity.example.test".into()]).unwrap(),
        registration: RegistrationPolicy::Open,
        email_verification: EmailVerificationPolicy::Optional,
        session_lifetime: SessionLifetime::new(86400).unwrap(),
        challenge_lifetime: ChallengeLifetime::new(1800).unwrap(),
        mail: MailSettings {
            host: String::new(),
            port: 587,
            username: String::new(),
            password: SecretString::default(),
            from_address: String::new(),
            from_name: "AsterGate".into(),
            security: SmtpSecurity::StartTls,
        },
    };
    assert!(settings.clone().validate().is_ok());
    let mut required = settings.clone();
    required.email_verification = EmailVerificationPolicy::Required;
    assert!(required.validate().is_err());
    let mut configured = settings.mail.clone();
    configured.host = "smtp.example.test".into();
    configured.from_address = "accounts@example.test".into();
    assert!(configured.clone().validate().is_ok());
    configured.password = SecretString::new("secret".into()).unwrap();
    assert!(configured.clone().validate().is_err());
    configured.username = "sender".into();
    assert!(configured.clone().validate().is_ok());
    configured.security = SmtpSecurity::LoopbackPlaintext;
    assert!(configured.validate().is_err());
}

#[test]
fn credentials_are_private_and_static_config_rejects_product_settings() {
    let mut config = AppConfig::default();
    config.security.master_key = "sensitive-master-key".into();
    for output in [toml::to_string(&config).unwrap(), format!("{config:?}")] {
        {
            let secret = "sensitive-master-key";
            assert!(!output.contains(secret));
        }
    }
    for input in [
        "[account]\nallow_registration = true",
        "[mail]\nsmtp_host = 'smtp.example.test'",
        "[security]\nsetup_token = 'obsolete'",
        "[account]\nmanagement_token = 'obsolete'",
        "[server]\nserver_port = 3003",
        "[database]\nconnection_string = 'sqlite://old.db'",
    ] {
        assert!(toml::from_str::<AppConfig>(input).is_err());
    }
    assert_eq!(
        format!("{:?}", SecretString::new("SMTP-password".into()).unwrap()),
        "[REDACTED]"
    );
}

#[test]
fn deployment_key_is_validated_and_defaults_have_full_entropy() {
    let first = SecurityConfig::default();
    let second = SecurityConfig::default();
    assert_ne!(first.master_key, second.master_key);
    assert_eq!(first.master_key.len(), 64);
    assert!(
        first
            .master_key
            .bytes()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
    );
    assert!(security::prepare(&first).is_ok());
    let mut config = SecurityConfig {
        master_key: hex::encode([7_u8; 32]),
    };
    assert!(security::prepare(&config).is_ok());
    config.master_key = "not-hex".into();
    assert!(security::prepare(&config).is_err());
}

#[test]
fn deployment_master_key_accepts_only_hex_and_rejects_malformed_input() {
    let bytes = [0xab_u8; 32];
    for encoded in [hex::encode(bytes), hex::encode_upper(bytes)] {
        assert_eq!(
            security::prepare(&SecurityConfig {
                master_key: encoded
            })
            .unwrap()
            .master_key,
            bytes
        );
    }
    for bad in [
        String::new(),
        "a".repeat(63),
        "a".repeat(65),
        "x".repeat(64),
        format!(" {}", hex::encode(bytes)),
        format!("{}\n", hex::encode(bytes)),
        "é".repeat(32),
        "BwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwc=".into(),
        "A".repeat(44),
    ] {
        assert!(security::prepare(&SecurityConfig { master_key: bad }).is_err());
    }
}
