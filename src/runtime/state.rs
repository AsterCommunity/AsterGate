//! Product runtime state.
//!
//! Store product-owned resources here. Clone only cheap handles such as `Arc`,
//! `DatabaseConnection`, `DbHandles`, and shared sender/metrics handles.

use std::sync::Arc;

use crate::config::AppConfig;

/// Shared runtime state passed to HTTP handlers and component factories.
#[derive(Clone)]
pub struct AppState {
    /// Static service configuration.
    pub config: Arc<AppConfig>,
    /// Database reader/writer handles.
    pub db_handles: aster_forge_db::DbHandles,
    /// Shared cache backend.
    pub cache: Arc<dyn aster_forge_cache::CacheBackend>,
    /// Cross-process runtime config reload handle.
    pub config_sync: aster_forge_config::ConfigSyncRuntime,
    pub runtime_config: Arc<crate::config::runtime_config::RuntimeConfig>,
    /// Shared metrics recorder.
    pub metrics: aster_forge_metrics::SharedMetricsRecorder,
    /// Product mail sender.
    pub mail_sender: Arc<dyn aster_forge_mail::MailSender>,
    pub account_security: Arc<crate::services::account::AccountSecurity>,
}

impl AppState {
    pub fn management_settings(
        &self,
    ) -> crate::services::account::Result<crate::services::system_settings::models::SystemSettings>
    {
        self.runtime_config.management_settings()
    }
    /// Returns the configured cache backend name.
    pub fn cache_backend_name(&self) -> &'static str {
        self.cache.backend_name()
    }

    /// Returns whether cross-process config reload notifications are enabled.
    pub fn config_sync_enabled(&self) -> bool {
        self.config_sync.enabled()
    }

    /// Returns the generated runtime instance ID.
    pub fn runtime_id(&self) -> &str {
        self.config_sync.runtime_id()
    }
}
