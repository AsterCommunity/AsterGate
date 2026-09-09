//! Metrics backend provided by AsterForge.
use aster_forge_runtime::{HealthCheckScope, SystemHealthReport};

pub fn record_health_report(scope: HealthCheckScope, report: &SystemHealthReport) {
    #[cfg(feature = "metrics")]
    report.record_metrics(
        scope.as_str(),
        &aster_forge_metrics::prometheus::PrometheusMetricsRecorder,
    );
    #[cfg(not(feature = "metrics"))]
    let _ = (scope, report);
}
