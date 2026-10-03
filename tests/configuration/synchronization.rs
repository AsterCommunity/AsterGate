use super::*;
use aster_forge_config::{
    ConfigChangeNotifier, ConfigCoreError, ConfigNotification, ConfigNotificationSource,
    ConfigReloadMessage, ConfigSyncRuntime, InMemoryConfigNotifier, SharedConfigChangeNotifier,
};
use std::{
    sync::atomic::{AtomicUsize, Ordering},
    time::Duration,
};
use tokio_util::sync::CancellationToken;

#[derive(Default)]
struct Recorder {
    notifier: InMemoryConfigNotifier,
    attempts: AtomicUsize,
    fail_attempts: AtomicUsize,
    subscribed: tokio::sync::Notify,
    messages: std::sync::Mutex<Vec<ConfigReloadMessage>>,
}
#[async_trait::async_trait]
impl ConfigChangeNotifier for Recorder {
    async fn publish_reload(&self, message: ConfigReloadMessage) -> aster_forge_config::Result<()> {
        let attempt = self.attempts.fetch_add(1, Ordering::SeqCst);
        if attempt < self.fail_attempts.load(Ordering::SeqCst) {
            return Err(ConfigCoreError::notification("test publish failure"));
        }
        self.messages.lock().unwrap().push(message.clone());
        self.notifier.publish_reload(message).await
    }
    async fn subscribe(&self) -> aster_forge_config::Result<ConfigNotification> {
        let subscription = self.notifier.subscribe().await?;
        self.subscribed.notify_one();
        Ok(subscription)
    }
}
fn sync(runtime_id: &str, recorder: Arc<Recorder>) -> ConfigSyncRuntime {
    let notifier: SharedConfigChangeNotifier = recorder;
    ConfigSyncRuntime::with_notifier_for_test("aster_gate", runtime_id, notifier)
}
async fn wait_title(state: &AppState, title: &str) {
    tokio::time::timeout(Duration::from_secs(3), async {
        while state.runtime_config.branding().unwrap().title.as_str() != title {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn committed_api_write_publishes_and_a_different_replica_reloads_from_database() {
    let (mut state, _database) = fixture().await;
    let recorder = Arc::new(Recorder::default());
    let mut peer = replica(&state).await;
    change(&state, "Connection ready").await;
    state.config_sync = sync("publisher", recorder.clone());
    peer.config_sync = sync("receiver", recorder.clone());
    let peer = Arc::new(peer);
    let shutdown = CancellationToken::new();
    let task = tokio::spawn(
        aster_gate::services::system_settings::synchronization::run_subscription(
            peer.clone(),
            shutdown.clone(),
        ),
    );
    recorder.subscribed.notified().await;
    wait_title(&peer, "Connection ready").await;
    change(&state, "Cluster Gate").await;
    wait_title(&peer, "Cluster Gate").await;
    assert_eq!(peer.runtime_config.authentication().unwrap().revision, 3);
    {
        let messages = recorder.messages.lock().unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].origin_runtime_id, "publisher");
        assert_eq!(messages[0].source, ConfigNotificationSource::Api);
        assert!(messages[0].keys.contains(&"branding_title".into()));
        assert!(!messages[0].encode().unwrap().contains(PASSWORD));
    }
    shutdown.cancel();
    task.await.unwrap().unwrap();
}

#[tokio::test]
async fn redis_transport_updates_an_independent_replica_after_initial_reconciliation() {
    if std::env::var("ASTER_TEST_CONFIG_SYNC_REDIS").as_deref() != Ok("1") {
        return;
    }
    use testcontainers::{
        GenericImage,
        core::{IntoContainerPort, WaitFor},
        runners::AsyncRunner,
    };
    let redis = GenericImage::new("redis", "7-alpine")
        .with_wait_for(WaitFor::message_on_stdout("Ready to accept connections"))
        .with_exposed_port(6379.tcp())
        .start()
        .await
        .unwrap();
    let port = redis.get_host_port_ipv4(6379.tcp()).await.unwrap();
    let config = aster_forge_config::ConfigSyncConfig {
        backend: "redis".into(),
        endpoint: format!("redis://127.0.0.1:{port}/0").into(),
        topic: format!("aster_gate.test.{}", uuid::Uuid::new_v4()),
    };
    let (mut state, _database) = fixture().await;
    let mut peer = replica(&state).await;
    change(&state, "Redis connection ready").await;
    state.config_sync =
        aster_forge_config::build_config_sync_runtime(&config, "aster_gate").unwrap();
    peer.config_sync =
        aster_forge_config::build_config_sync_runtime(&config, "aster_gate").unwrap();
    let peer = Arc::new(peer);
    let shutdown = CancellationToken::new();
    let task = tokio::spawn(
        aster_gate::services::system_settings::synchronization::run_subscription(
            peer.clone(),
            shutdown.clone(),
        ),
    );
    wait_title(&peer, "Redis connection ready").await;
    change(&state, "Redis published Gate").await;
    wait_title(&peer, "Redis published Gate").await;
    assert_eq!(peer.runtime_config.authentication().unwrap().revision, 3);
    shutdown.cancel();
    tokio::time::timeout(Duration::from_secs(3), task)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
}

#[tokio::test]
async fn connection_reconciliation_recovers_changes_made_before_subscription() {
    let (mut state, _database) = fixture().await;
    let recorder = Arc::new(Recorder::default());
    state.config_sync = sync("publisher", recorder.clone());
    let mut peer = replica(&state).await;
    peer.config_sync = sync("receiver", recorder.clone());
    change(&state, "Before subscription").await;
    assert_eq!(
        peer.runtime_config.branding().unwrap().title.as_str(),
        "AsterGate"
    );
    let peer = Arc::new(peer);
    let shutdown = CancellationToken::new();
    let task = tokio::spawn(
        aster_gate::services::system_settings::synchronization::run_subscription(
            peer.clone(),
            shutdown.clone(),
        ),
    );
    wait_title(&peer, "Before subscription").await;
    shutdown.cancel();
    task.await.unwrap().unwrap();
}

#[tokio::test]
async fn publication_failure_is_bounded_and_does_not_report_a_committed_change_as_failed() {
    let (mut state, _database) = fixture().await;
    let recorder = Arc::new(Recorder::default());
    recorder.fail_attempts.store(usize::MAX, Ordering::SeqCst);
    state.config_sync = sync("publisher", recorder.clone());
    change(&state, "Committed without publication").await;
    assert_eq!(recorder.attempts.load(Ordering::SeqCst), 3);
    assert_eq!(
        state.runtime_config.branding().unwrap().title.as_str(),
        "Committed without publication"
    );
    let peer = replica(&state).await;
    assert_eq!(
        peer.runtime_config.branding().unwrap().title.as_str(),
        "Committed without publication"
    );
}

#[tokio::test]
async fn transaction_failure_never_updates_snapshot_or_publishes() {
    let (mut state, _database) = fixture().await;
    let recorder = Arc::new(Recorder::default());
    state.config_sync = sync("publisher", recorder.clone());
    use sea_orm::ConnectionTrait;
    let principal = operator(&state).await;
    state.db_handles.writer().execute_unprepared("CREATE TRIGGER reject_audit BEFORE INSERT ON audit_logs BEGIN SELECT RAISE(ABORT, 'injected failure'); END").await.unwrap();
    let mut settings = state.management_settings().unwrap();
    settings.branding.title =
        aster_gate::services::system_settings::branding::BrandingTitle::parse("Uncommitted Gate")
            .unwrap();
    assert!(
        aster_gate::services::system_setup::update_settings(
            &state,
            &principal,
            settings,
            Some(PASSWORD.into())
        )
        .await
        .is_err()
    );
    assert_eq!(recorder.attempts.load(Ordering::SeqCst), 0);
    assert_eq!(
        state.runtime_config.branding().unwrap().title.as_str(),
        "AsterGate"
    );
}

#[tokio::test]
async fn transient_publication_errors_retry_and_disabled_subscription_cancels_cleanly() {
    let (mut state, _database) = fixture().await;
    let recorder = Arc::new(Recorder::default());
    recorder.fail_attempts.store(2, Ordering::SeqCst);
    state.config_sync = sync("publisher", recorder.clone());
    change(&state, "Retried Gate").await;
    assert_eq!(recorder.attempts.load(Ordering::SeqCst), 3);
    assert_eq!(recorder.messages.lock().unwrap().len(), 1);
    state.config_sync = ConfigSyncRuntime::disabled_for_test("aster_gate");
    let shutdown = CancellationToken::new();
    let task = tokio::spawn(
        aster_gate::services::system_settings::synchronization::run_subscription(
            Arc::new(state),
            shutdown.clone(),
        ),
    );
    shutdown.cancel();
    tokio::time::timeout(Duration::from_secs(1), task)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
}
