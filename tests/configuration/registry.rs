use aster_gate::config::{definitions::*, system_config::CONFIG_REGISTRY};
use std::collections::HashMap;

#[test]
fn registry_normalizes_domain_values_and_rejects_invalid_boundaries() {
    let lookup = HashMap::<String, String>::new();
    assert_eq!(
        CONFIG_REGISTRY
            .normalize_value(&lookup, BRANDING_TITLE_KEY, "  Custom Gate  ")
            .unwrap(),
        "Custom Gate"
    );
    assert_eq!(
        CONFIG_REGISTRY
            .normalize_value(&lookup, BRANDING_DESCRIPTION_KEY, " ")
            .unwrap(),
        "Account and identity management"
    );
    assert_eq!(CONFIG_REGISTRY.normalize_value(&lookup, PUBLIC_SITE_URLS_KEY, r#"[" HTTPS://Gate.EXAMPLE.test:443/ ","https://panel.example.test","https://gate.example.test"]"#).unwrap(), r#"["https://gate.example.test","https://panel.example.test"]"#);
    assert_eq!(
        CONFIG_REGISTRY
            .normalize_value(&lookup, MAIL_SMTP_HOST_KEY, " SMTP.Example.test ")
            .unwrap(),
        "smtp.example.test"
    );
    for (key, value) in [
        (PUBLIC_SITE_URLS_KEY, r#"["https://*.example.test"]"#),
        (PUBLIC_SITE_URLS_KEY, r#"["https://example.test/path"]"#),
        (PUBLIC_SITE_URLS_KEY, "https://example.test"),
        (BRANDING_TITLE_KEY, "bad\nname"),
        (ACCOUNT_REGISTRATION_KEY, "true"),
        (ACCOUNT_EMAIL_VERIFICATION_KEY, "force"),
        (ACCOUNT_SESSION_TTL_KEY, "299"),
        (ACCOUNT_SESSION_TTL_KEY, "2592001"),
        (ACCOUNT_SESSION_TTL_KEY, "300.5"),
        (ACCOUNT_CHALLENGE_TTL_KEY, "59"),
        (MAIL_SMTP_PORT_KEY, "0"),
        (MAIL_SMTP_PORT_KEY, "65536"),
        (MAIL_SMTP_SECURITY_KEY, "plaintext"),
        (MAIL_FROM_ADDRESS_KEY, "not-an-email"),
        (MAIL_SMTP_HOST_KEY, "smtp.example.test/host"),
        (SETTINGS_REVISION_KEY, "1.5"),
    ] {
        assert!(
            CONFIG_REGISTRY
                .normalize_value(&lookup, key, value)
                .is_err(),
            "{key}: {value}"
        );
    }
    let initialized = HashMap::from([(SETUP_COMPLETED_KEY.to_string(), "true".to_string())]);
    assert!(
        CONFIG_REGISTRY
            .normalize_value(&initialized, PUBLIC_SITE_URLS_KEY, "[]")
            .is_err()
    );
}

#[test]
fn registry_metadata_and_cross_field_validation_match_runtime_contract() {
    CONFIG_REGISTRY.validate_unique_keys().unwrap();
    CONFIG_REGISTRY
        .validate_categories(&["site", "account", "mail", "setup"])
        .unwrap();
    assert_eq!(CONFIG_REGISTRY.default_seed_records().unwrap().len(), 16);
    assert!(!CONFIG_REGISTRY.contains_key("public_origin"));
    assert!(
        CONFIG_REGISTRY
            .get(MAIL_SMTP_PASSWORD_KEY)
            .unwrap()
            .is_sensitive
    );
    assert!(
        CONFIG_REGISTRY
            .get(BRANDING_TITLE_KEY)
            .unwrap()
            .visibility
            .visible_to_public()
    );
    let mut lookup = HashMap::from([
        (ACCOUNT_REGISTRATION_KEY.to_string(), "open".to_string()),
        (
            ACCOUNT_EMAIL_VERIFICATION_KEY.to_string(),
            "required".to_string(),
        ),
        (MAIL_SMTP_HOST_KEY.to_string(), String::new()),
    ]);
    assert!(
        CONFIG_REGISTRY
            .normalize_value(&lookup, ACCOUNT_REGISTRATION_KEY, "open")
            .is_err()
    );
    lookup.insert(MAIL_SMTP_HOST_KEY.into(), "smtp.example.test".into());
    assert!(
        CONFIG_REGISTRY
            .normalize_value(&lookup, ACCOUNT_REGISTRATION_KEY, "open")
            .is_ok()
    );
    lookup.insert(ACCOUNT_EMAIL_VERIFICATION_KEY.into(), "optional".into());
    lookup.insert(MAIL_SMTP_HOST_KEY.into(), String::new());
    assert!(
        CONFIG_REGISTRY
            .normalize_value(&lookup, ACCOUNT_EMAIL_VERIFICATION_KEY, "required")
            .is_err()
    );
}
