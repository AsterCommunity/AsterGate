//! Configuration loading and shared runtime access.
//!
//! The concrete static schema lives in `schema`, matching the structure used by Aster services
//! such as AsterYggdrasil. Product-specific runtime-editable configuration can grow into sibling
//! modules later without forcing callers to change imports.

pub mod definitions;
mod loader;
mod normalizers;
pub mod runtime_config;
mod schema;
pub mod security;
pub mod system_config;

pub use loader::{CONFIG_ENV_VAR, DEFAULT_CONFIG_PATH, load};
pub use schema::{AppConfig, DatabaseConfig};
pub use security::SecurityConfig;
