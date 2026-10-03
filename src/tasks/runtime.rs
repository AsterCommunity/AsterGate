//! Background task runtime component integration.
//!
//! Keep product task kind, payload, result, lane policy, and execution bodies in this module tree.
//! Forge owns the handle collection and shutdown mechanics.

/// Creates the background task component used by the product entrypoint.
pub fn background_tasks_component(
    state: std::sync::Arc<crate::runtime::AppState>,
) -> aster_forge_tasks::BackgroundTaskRuntimeComponentFromShutdown<
    impl FnOnce(tokio_util::sync::CancellationToken) -> aster_forge_tasks::BackgroundTasks,
> {
    aster_forge_tasks::background_task_component_from_shutdown(move |shutdown_token| {
        // Register spawned workers here. Use the shutdown token for long-running loops.
        let mut tasks = aster_forge_tasks::BackgroundTasks::with_shutdown_token(shutdown_token);
        let synchronization_state = state.clone();
        let synchronization_shutdown = tasks.shutdown_token();
        tasks.push(async move {
            if crate::services::system_settings::synchronization::run_subscription(
                synchronization_state,
                synchronization_shutdown,
            )
            .await
            .is_err()
            {
                tracing::error!("configuration subscription worker stopped unexpectedly");
            }
        });
        if let Some(task) = state
            .metrics
            .system_metrics_updater_task(tasks.shutdown_token())
        {
            tasks.push(task);
        }
        let shutdown = tasks.shutdown_token();
        tasks.push(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(5));
            loop {
                tokio::select! {
                    _ = shutdown.cancelled() => break,
                    _ = interval.tick() => {
                        if crate::services::mail_outbox_service::dispatch_due_with(&state).await.is_err() {
                            tracing::warn!("account mail dispatch failed; durable outbox will retry");
                        }
                        if crate::services::account::cleanup(&state).await.is_err() {
                            tracing::warn!("account retention cleanup failed");
                        }
                    }
                }
            }
        });
        tasks
    })
}
