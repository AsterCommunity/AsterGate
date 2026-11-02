//! Runtime metrics recorder initialization.

use aster_forge_metrics::SharedMetricsRecorder;

/// Creates the runtime metrics recorder for this build.
pub fn create_metrics_recorder() -> crate::errors::Result<SharedMetricsRecorder> {
    #[cfg(feature = "metrics")]
    {
        let provider = aster_forge_metrics::MetricsProvider::builder()
            .with_prometheus()
            .and_then(|builder| builder.build())
            .map_err(|_| {
                crate::errors::AppError::Runtime("cannot initialize metrics provider".into())
            })?;
        let recorder =
            aster_forge_metrics::InfrastructureMetricsRecorder::new(provider).map_err(|_| {
                crate::errors::AppError::Runtime("cannot register infrastructure metrics".into())
            })?;
        Ok(std::sync::Arc::new(recorder))
    }

    #[cfg(not(feature = "metrics"))]
    {
        Ok(aster_forge_metrics::NoopMetrics::arc())
    }
}
