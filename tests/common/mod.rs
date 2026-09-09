//! Integration test helpers.
use aster_forge_cache::CacheConfig;
use aster_forge_test::temp::SqliteTestDatabase;

pub async fn setup() -> (aster_gate::runtime::AppState, SqliteTestDatabase) {
    let database = SqliteTestDatabase::new("service-state");
    let mut config = aster_gate::config::AppConfig::default();
    config.database.url = database.url().into();
    config.cache = CacheConfig::default();
    config.logging.file = String::new();
    let state = aster_gate::runtime::assembly::prepare_state(config)
        .await
        .expect("runtime state should prepare");
    (state, database)
}

#[macro_export]
macro_rules! create_test_app {
    ($state:expr) => {{ aster_gate::api::router(std::sync::Arc::new($state)) }};
}
