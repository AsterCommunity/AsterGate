//! Forge database binding for product runtime configuration.
use super::definitions::CONFIG_DEFINITIONS;
use aster_forge_config::ConfigRegistry;

pub static CONFIG_REGISTRY: ConfigRegistry = ConfigRegistry::new(&CONFIG_DEFINITIONS);
pub const CONFIG_STORE: aster_forge_db::system_config::SystemConfigDbBinding =
    aster_forge_db::system_config::SystemConfigDbBinding::new(&CONFIG_REGISTRY, &[]);
