//! Post-commit configuration publication, subscription and authoritative reconciliation.
use crate::{db::repository::system_config_repo::ConfigSnapshot, runtime::AppState};
use aster_forge_config::{ConfigCoreError, ConfigNotificationSource};
use std::{sync::Arc, time::Duration};
use tokio_util::sync::CancellationToken;

pub async fn after_commit(state: &AppState, snapshot: ConfigSnapshot) {
    if state.runtime_config.apply_snapshot(snapshot).is_err() {
        tracing::error!("committed configuration could not be applied locally");
    }
    for attempt in 0..3 {
        let result = state
            .config_sync
            .publish_reload(
                crate::config::system_config::CONFIG_REGISTRY
                    .definitions()
                    .iter()
                    .map(|definition| definition.key),
                ConfigNotificationSource::Api,
            )
            .await;
        if result.is_ok() {
            return;
        }
        if attempt < 2 {
            tokio::time::sleep(Duration::from_millis(50 << attempt)).await;
        }
    }
    state
        .metrics
        .record_application_event("config_sync", "publish", "error");
    tracing::warn!(
        "configuration committed; reload publication failed after three attempts; peer replicas may require resynchronization"
    );
}

pub async fn run_subscription(
    state: Arc<AppState>,
    shutdown: CancellationToken,
) -> aster_forge_config::Result<()> {
    let runtime = state.config_sync.clone();
    let reconcile_state = state.clone();
    runtime
        .run_reload_subscription_with_reconcile(
            shutdown,
            move || {
                let state = reconcile_state.clone();
                async move {
                    state
                        .runtime_config
                        .reload(state.db_handles.writer())
                        .await
                        .map_err(|_| ConfigCoreError::store("configuration reconciliation failed"))
                }
            },
            move |_message| {
                let state = state.clone();
                async move {
                    state
                        .runtime_config
                        .reload(state.db_handles.writer())
                        .await
                        .map_err(|_| ConfigCoreError::store("configuration reload failed"))
                }
            },
        )
        .await
}
