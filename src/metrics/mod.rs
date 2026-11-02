//! Metrics backend provided by AsterForge.
use aster_forge_runtime::{HealthCheckScope, SystemHealthReport};

pub fn record_health_report(
    recorder: &aster_forge_metrics::SharedMetricsRecorder,
    scope: HealthCheckScope,
    report: &SystemHealthReport,
) {
    #[cfg(feature = "metrics")]
    recorder.record_health(scope.as_str(), report);
    #[cfg(not(feature = "metrics"))]
    let _ = (recorder, scope, report);
}
