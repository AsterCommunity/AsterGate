use aster_forge_test::temp::SqliteTestDatabase;
use aster_gate::{runtime::AppState, services::account::AccountError};
use std::sync::Arc;
mod registry;
mod runtime;
mod synchronization;

const PASSWORD: &str = "correct horse battery staple";
async fn fixture() -> (AppState, SqliteTestDatabase) {
    let database = SqliteTestDatabase::new("runtime-config");
    let mut config = aster_gate::config::AppConfig::default();
    config.database.url = database.url().into();
    config.security.master_key = hex::encode([7_u8; 32]);
    let state = aster_gate::runtime::assembly::prepare_state(config)
        .await
        .unwrap();
    let input: aster_gate::api::dto::system::SetupRequest = serde_json::from_value(serde_json::json!({
        "site_urls":["https://primary.example.test", "https://secondary.example.test"],
        "initial_account":{"username":"operator","email":"operator@example.test","display_name":"Operator", "password":PASSWORD}
    })).unwrap();
    aster_gate::services::system_setup::initialize(
        &state,
        input.command().unwrap(),
        "configuration test",
    )
    .await
    .unwrap();
    (state, database)
}

async fn replica(state: &AppState) -> AppState {
    let mut replica = state.clone();
    replica.runtime_config = Arc::new(aster_gate::config::runtime_config::RuntimeConfig::new(
        aster_gate::db::repository::system_config_repo::load_snapshot(state.db_handles.writer())
            .await
            .unwrap(),
        [7_u8; 32],
    ));
    replica
}

async fn operator(state: &AppState) -> aster_gate::services::account::models::Principal {
    let login =
        aster_gate::services::account::login(state, "operator", PASSWORD.into(), "config test")
            .await
            .unwrap();
    aster_gate::services::account::authenticate(state, &login.session_token)
        .await
        .unwrap()
}

async fn change(state: &AppState, title: &str) {
    let mut settings = state.management_settings().unwrap();
    settings.branding.title =
        aster_gate::services::system_settings::branding::BrandingTitle::parse(title).unwrap();
    aster_gate::services::system_setup::update_settings(
        state,
        &operator(state).await,
        settings,
        Some(PASSWORD.into()),
    )
    .await
    .unwrap();
}
