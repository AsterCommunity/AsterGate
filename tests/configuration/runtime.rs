use super::*;
use aster_gate::{config::definitions::*, db::repository::system_config_repo as repository};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, sea_query::Expr};

async fn corrupt(state: &AppState, key: &str, value: &str) {
    use aster_forge_db::system_config::{Column, Entity};
    Entity::update_many()
        .col_expr(Column::Value, Expr::value(value))
        .filter(Column::Key.eq(key))
        .exec(state.db_handles.writer())
        .await
        .unwrap();
    state
        .runtime_config
        .reload(state.db_handles.writer())
        .await
        .unwrap();
}

#[tokio::test]
async fn cached_projections_do_not_query_database_or_redecrypt_mail() {
    let (state, _database) = fixture().await;
    let expected = state.management_settings().unwrap();
    let authentication = state.runtime_config.authentication().unwrap();
    let branding = state.runtime_config.branding().unwrap();
    state.db_handles.writer().clone().close().await.unwrap();
    for _ in 0..20 {
        assert_eq!(
            state.runtime_config.authentication().unwrap(),
            authentication
        );
        assert_eq!(state.runtime_config.branding().unwrap(), branding);
        assert_eq!(state.management_settings().unwrap(), expected);
    }
}

#[tokio::test]
async fn corrupt_smtp_ciphertext_does_not_disable_login_or_public_branding() {
    let (state, _database) = fixture().await;
    corrupt(&state, MAIL_SMTP_PASSWORD_KEY, "invalid ciphertext").await;
    assert!(state.runtime_config.mail().is_err());
    assert!(state.management_settings().is_err());
    assert!(state.runtime_config.branding().is_ok());
    assert!(state.runtime_config.authentication().is_ok());
    assert!(
        aster_gate::services::account::login(&state, "operator", PASSWORD.into(), "test")
            .await
            .is_ok()
    );
    let router = aster_gate::api::router(Arc::new(state));
    use tower::ServiceExt;
    for (path, status) in [
        ("/health", axum::http::StatusCode::OK),
        ("/health/ready", axum::http::StatusCode::SERVICE_UNAVAILABLE),
        ("/api/v1/public/frontend-config", axum::http::StatusCode::OK),
    ] {
        assert_eq!(
            router
                .clone()
                .oneshot(
                    axum::http::Request::builder()
                        .uri(path)
                        .body(axum::body::Body::empty())
                        .unwrap()
                )
                .await
                .unwrap()
                .status(),
            status
        );
    }
}

#[tokio::test]
async fn invalid_authentication_policy_fails_closed_without_breaking_branding() {
    let (state, _database) = fixture().await;
    corrupt(
        &state,
        PUBLIC_SITE_URLS_KEY,
        r#"["http://evil.example.test"]"#,
    )
    .await;
    assert!(state.runtime_config.authentication().is_err());
    assert!(state.runtime_config.branding().is_ok());
    assert!(
        aster_gate::services::account::login(&state, "operator", PASSWORD.into(), "test")
            .await
            .is_err()
    );
}

#[tokio::test]
async fn older_snapshots_do_not_roll_back_local_committed_settings() {
    let (state, _database) = fixture().await;
    let old = repository::load_snapshot(state.db_handles.writer())
        .await
        .unwrap();
    change(&state, "Updated Gate").await;
    state.runtime_config.apply_snapshot(old).unwrap();
    assert_eq!(state.runtime_config.authentication().unwrap().revision, 2);
    assert_eq!(
        state.runtime_config.branding().unwrap().title.as_str(),
        "Updated Gate"
    );
}

#[tokio::test]
async fn wrong_deployment_key_only_breaks_encrypted_mail_and_correct_key_restores_it() {
    let (state, _database) = fixture().await;
    let mut settings = state.management_settings().unwrap();
    settings.mail = aster_gate::services::system_settings::mail::MailSettings {
        host: "smtp.example.test".into(),
        port: 587,
        username: "sender".into(),
        password: aster_gate::services::system_settings::mail::SecretString::new(
            "mail secret".into(),
        )
        .unwrap(),
        from_address: "accounts@example.test".into(),
        from_name: "Gate".into(),
        security: aster_gate::services::system_settings::mail::SmtpSecurity::StartTls,
    };
    aster_gate::services::system_setup::update_settings(
        &state,
        &operator(&state).await,
        settings,
        Some(PASSWORD.into()),
    )
    .await
    .unwrap();
    let source = repository::load_snapshot(state.db_handles.writer())
        .await
        .unwrap();
    let wrong = aster_gate::config::runtime_config::RuntimeConfig::new(source.clone(), [8_u8; 32]);
    assert!(matches!(wrong.mail(), Err(AccountError::Security)));
    assert!(wrong.authentication().is_ok());
    assert!(wrong.branding().is_ok());
    let correct = aster_gate::config::runtime_config::RuntimeConfig::new(source, [7_u8; 32]);
    assert!(correct.mail().unwrap().configured());
}
