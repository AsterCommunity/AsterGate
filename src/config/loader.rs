//! Static configuration loader.
//!
//! The loader keeps generated services convenient for local development while supporting real
//! deployment files. Like AsterDrive and AsterYggdrasil, first startup writes `data/config.toml`,
//! then loads that file and overlays `ASTER__...` environment variables.

use std::path::{Path, PathBuf};

use crate::errors::{AppError, Result};

/// Environment variable used to override the static configuration file path.
pub const CONFIG_ENV_VAR: &str = "ASTER_CONFIG";

/// Default static configuration file path.
pub const DEFAULT_CONFIG_PATH: &str = "data/config.toml";

/// Loads static configuration from `ASTER_CONFIG` or `data/config.toml`.
pub fn load() -> Result<super::AppConfig> {
    let path = config_path();
    ensure_default_config_exists(&path, &super::AppConfig::default())?;
    ensure_deployment_key(&path)?;

    let mut builder = ::config::Config::builder()
        .add_source(::config::File::from(path.as_path()).required(false))
        .add_source(
            ::config::Environment::with_prefix("ASTER")
                .separator("__")
                .try_parsing(true),
        );
    // Hex keys containing only digits must remain strings, never environment-inferred floats.
    if let Ok(value) = std::env::var("ASTER__SECURITY__MASTER_KEY") {
        builder = builder.set_override("security.master_key", value)?;
    }
    let mut loaded = builder.build()?.try_deserialize::<super::AppConfig>()?;
    normalize_paths(&mut loaded, &path)?;
    tracing::info!(path = %path.display(), "loaded configuration file");
    Ok(loaded)
}

fn config_path() -> PathBuf {
    std::env::var_os(CONFIG_ENV_VAR)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(DEFAULT_CONFIG_PATH))
}

fn ensure_default_config_exists(path: &Path, default: &super::AppConfig) -> Result<()> {
    if path.exists() {
        return Ok(());
    }

    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent)?;
    }

    let toml = toml::to_string_pretty(default).map_err(|error| {
        AppError::Config(format!(
            "failed to serialize default configuration: {error}"
        ))
    })?;
    let content = format!(
        "# {service} configuration file\n\
         # Generated on first startup; edit as needed.\n\
         # Relative paths are resolved against the directory containing this file.\n\n\
         {toml}",
        service = env!("CARGO_PKG_NAME")
    );
    std::fs::write(path, content)?;
    Ok(())
}

fn normalize_paths(config: &mut super::AppConfig, config_path: &Path) -> Result<()> {
    let base_dir = std::env::current_dir()?;
    let raw_config_dir = config_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let config_dir = if raw_config_dir.is_absolute() {
        raw_config_dir.to_path_buf()
    } else {
        base_dir.join(raw_config_dir)
    };

    config.server.temp_dir = aster_forge_utils::paths::resolve_config_relative_path(
        &base_dir,
        &config_dir,
        &config.server.temp_dir,
    )
    .map_err(|error| AppError::Config(error.to_string()))?;
    config.logging.file = if config.logging.file.is_empty() {
        String::new()
    } else {
        aster_forge_utils::paths::resolve_config_relative_path(
            &base_dir,
            &config_dir,
            &config.logging.file,
        )
        .map_err(|error| AppError::Config(error.to_string()))?
    };
    if let Some(database_url) = config.database.url.as_url_mut() {
        *database_url = aster_forge_utils::paths::resolve_config_relative_sqlite_url(
            &base_dir,
            &config_dir,
            database_url,
        )
        .map_err(|error| AppError::Config(error.to_string()))?;
    }
    Ok(())
}

fn ensure_deployment_key(path: &Path) -> Result<()> {
    if std::env::var_os("ASTER__SECURITY__MASTER_KEY").is_some() {
        return Ok(());
    }
    let content = std::fs::read_to_string(path)?;
    let mut doc = content
        .parse::<toml_edit::DocumentMut>()
        .map_err(|_| AppError::Config("invalid config file".into()))?;
    let table = doc
        .as_table_mut()
        .entry("security")
        .or_insert(toml_edit::Item::Table(toml_edit::Table::new()))
        .as_table_mut()
        .ok_or_else(|| AppError::Config("security must be a table".into()))?;
    if let Some(value) = table.get("master_key") {
        let value = value
            .as_str()
            .ok_or_else(|| AppError::Config("master_key must be a string".into()))?;
        super::security::decode_master_key(value)?;
        return Ok(());
    }
    {
        table.insert(
            "master_key",
            toml_edit::value(super::SecurityConfig::default().master_key),
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
        }
        std::fs::write(path, doc.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_default_file_uses_config_relative_paths() {
        let directory = aster_forge_test::temp::TestTempDir::new("loader-default");
        let path = directory.join("config.toml");

        ensure_default_config_exists(&path, &crate::config::AppConfig::default())
            .expect("write default config");
        let generated = std::fs::read_to_string(&path).expect("read default config");

        assert!(generated.contains(r#"temp_dir = ".tmp""#));
        assert!(generated.contains(r#"url = "sqlite://aster_gate.db?mode=rwc""#));
    }

    #[test]
    fn deployment_key_backfill_is_stable_and_preserves_comments() {
        let directory = aster_forge_test::temp::TestTempDir::new("loader-master-key");
        let path = directory.join("config.toml");
        std::fs::write(&path, "# deployment comment\n[server]\nport = 3210\n").unwrap();
        ensure_deployment_key(&path).unwrap();
        let first = std::fs::read_to_string(&path).unwrap();
        ensure_deployment_key(&path).unwrap();
        let second = std::fs::read_to_string(&path).unwrap();
        assert_eq!(first, second);
        assert!(second.starts_with("# deployment comment"));
        let loaded: crate::config::AppConfig = toml::from_str(&second).unwrap();
        assert_eq!(loaded.server.port, 3210);
        assert!(crate::config::security::prepare(&loaded.security).is_ok());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
    }

    #[test]
    fn invalid_existing_master_key_is_never_replaced() {
        let directory = aster_forge_test::temp::TestTempDir::new("loader-invalid-key");
        let path = directory.join("config.toml");
        let content = "[security]\nmaster_key = 'broken'\n";
        std::fs::write(&path, content).unwrap();
        assert!(ensure_deployment_key(&path).is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), content);
    }

    #[test]
    fn base64_config_key_is_rejected_and_not_rewritten() {
        let directory = aster_forge_test::temp::TestTempDir::new("loader-unsupported-key");
        let path = directory.join("config.toml");
        let content = "[security]\nmaster_key = 'BwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwc='\n";
        std::fs::write(&path, content).unwrap();
        assert!(ensure_deployment_key(&path).is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), content);
    }

    #[test]
    fn valid_uppercase_hex_key_is_preserved_without_rewrite() {
        let directory = aster_forge_test::temp::TestTempDir::new("loader-existing-hex");
        let path = directory.join("config.toml");
        let content = format!(
            "# keep\n[security]\nmaster_key = '{}'\n",
            hex::encode_upper([0xab_u8; 32])
        );
        std::fs::write(&path, &content).unwrap();
        ensure_deployment_key(&path).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), content);
    }

    #[test]
    fn structured_database_credentials_preserve_base_url_and_secrets_stay_out_of_serde() {
        let directory = aster_forge_test::temp::TestTempDir::new_in(
            std::env::current_dir()
                .expect("resolve current dir")
                .join("target")
                .join("config-loader-tests"),
            "loader-credentials",
        );
        let path = directory.join("config.toml");
        std::fs::write(
            &path,
            r#"
[database]
url = { base_url = "postgres://db.example:5432/app", username = "raw-user@example.com", password = "raw#[]{}secret" }
"#,
        )
        .expect("write credential config");

        let mut loaded = ::config::Config::builder()
            .add_source(::config::File::from(path.as_path()))
            .build()
            .expect("build credential config")
            .try_deserialize::<crate::config::AppConfig>()
            .expect("deserialize credential config");
        normalize_paths(&mut loaded, &path).expect("normalize credential config");

        assert_eq!(
            loaded.database.url,
            aster_forge_db::DatabaseUrl::credentials(
                "postgres://db.example:5432/app",
                Some("raw-user@example.com".to_string()),
                Some("raw#[]{}secret".to_string()),
            )
        );

        let serialized = toml::to_string(&loaded).expect("serialize loaded config");
        assert!(serialized.contains("postgres://db.example:5432/app"));
        assert!(!serialized.contains("raw-user@example.com"));
        assert!(!serialized.contains("raw#[]{}secret"));
        assert!(!serialized.contains("raw%23%5B%5D"));
    }
}
