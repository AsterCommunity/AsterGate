//! Static configuration loader integration tests.

use aster_forge_test::temp::TestTempDir;
use std::fs;
use std::sync::{Mutex, MutexGuard};

static ENV_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn default_config_uses_generated_data_paths() {
    let config = aster_gate::config::AppConfig::default();

    assert_eq!(config.server.temp_dir, ".tmp");
    assert_eq!(
        config.database.url.as_url(),
        Some("sqlite://aster_gate.db?mode=rwc")
    );
    assert!(config.logging.file.is_empty());
}

#[test]
fn default_config_file_is_created_under_data_dir() {
    let _guard = EnvGuard::capture();
    let directory = unique_project_temp_dir();
    let dir = directory.path();
    fs::create_dir_all(dir).expect("create temp config dir");
    let config_path = dir.join("data").join("config.toml");

    unsafe {
        std::env::set_var(aster_gate::config::CONFIG_ENV_VAR, &config_path);
    }
    let loaded = aster_gate::config::load().expect("load config");
    let generated = fs::read_to_string(&config_path).expect("read generated config");

    assert_eq!(
        loaded.server.temp_dir,
        runtime_relative_path(dir.join("data").join(".tmp"))
    );
    let expected_database_url = format!(
        "sqlite://{}?mode=rwc",
        runtime_relative_path(dir.join("data").join("aster_gate.db"))
    );
    assert_eq!(
        loaded.database.url.as_url(),
        Some(expected_database_url.as_str())
    );
    assert!(config_path.exists());
    assert!(generated.contains("# aster_gate configuration file"));
    assert!(generated.contains(r#"temp_dir = ".tmp""#));
    assert!(generated.contains(r#"url = "sqlite://aster_gate.db?mode=rwc""#));
    assert!(generated.contains(r#"file = """#));
    assert!(generated.contains("enable_rotation = false"));
}

#[test]
fn config_file_overrides_are_loaded_and_relative_paths_resolve_from_config_dir() {
    let _guard = EnvGuard::capture();
    let directory = unique_project_temp_dir();
    let dir = directory.path();
    let config_path = dir.join("data").join("config.toml");
    fs::create_dir_all(
        config_path
            .parent()
            .expect("config path should have parent"),
    )
    .expect("create temp config dir");
    fs::write(
        &config_path,
        r#"
[server]
temp_dir = ".tmp"

[database]
url = "sqlite://custom.db?mode=rwc"

[logging]
file = "service.log"
"#,
    )
    .expect("write config");

    unsafe {
        std::env::set_var(aster_gate::config::CONFIG_ENV_VAR, &config_path);
    }
    let loaded = aster_gate::config::load().expect("load config");

    assert_eq!(
        loaded.server.temp_dir,
        runtime_relative_path(dir.join("data").join(".tmp"))
    );
    let expected_database_url = format!(
        "sqlite://{}?mode=rwc",
        runtime_relative_path(dir.join("data").join("custom.db"))
    );
    assert_eq!(
        loaded.database.url.as_url(),
        Some(expected_database_url.as_str())
    );
    assert_eq!(
        loaded.logging.file,
        runtime_relative_path(dir.join("data").join("service.log"))
    );
}

#[test]
fn environment_overrides_static_config() {
    let _guard = EnvGuard::capture();

    unsafe {
        std::env::remove_var(aster_gate::config::CONFIG_ENV_VAR);
        std::env::set_var("ASTER__SERVER__HOST", "0.0.0.0");
    }

    let loaded = aster_gate::config::load().expect("load config");

    assert_eq!(loaded.server.host, "0.0.0.0");
}

struct EnvGuard {
    _lock: MutexGuard<'static, ()>,
    config_path: Option<std::ffi::OsString>,
    server_host: Option<std::ffi::OsString>,
    master_key: Option<std::ffi::OsString>,
    session_key: Option<std::ffi::OsString>,
    session_issuer: Option<std::ffi::OsString>,
}

impl EnvGuard {
    fn capture() -> Self {
        let lock = ENV_LOCK.lock().expect("env lock poisoned");
        Self {
            _lock: lock,
            config_path: std::env::var_os(aster_gate::config::CONFIG_ENV_VAR),
            server_host: std::env::var_os("ASTER__SERVER__HOST"),
            master_key: std::env::var_os("ASTER__SECURITY__MASTER_KEY"),
            session_key: std::env::var_os("ASTER__SESSION__SIGNING_KEY"),
            session_issuer: std::env::var_os("ASTER__SESSION__ISSUER"),
        }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        unsafe {
            restore_env_var(aster_gate::config::CONFIG_ENV_VAR, self.config_path.clone());
            restore_env_var("ASTER__SERVER__HOST", self.server_host.clone());
            restore_env_var("ASTER__SECURITY__MASTER_KEY", self.master_key.clone());
            restore_env_var("ASTER__SESSION__SIGNING_KEY", self.session_key.clone());
            restore_env_var("ASTER__SESSION__ISSUER", self.session_issuer.clone());
        }
    }
}

#[test]
fn numeric_hex_environment_key_is_preserved_exactly_and_not_written_to_file() {
    let _guard = EnvGuard::capture();
    let directory = unique_project_temp_dir();
    let path = directory.join("config.toml");
    let key = "07".repeat(32);
    unsafe {
        std::env::set_var(aster_gate::config::CONFIG_ENV_VAR, &path);
        std::env::set_var("ASTER__SECURITY__MASTER_KEY", &key);
    }
    let loaded = aster_gate::config::load().unwrap();
    assert_eq!(loaded.security.master_key, key);
    assert!(aster_gate::config::security::prepare(&loaded.security).is_ok());
    assert!(!fs::read_to_string(path).unwrap().contains(&key));
}

#[test]
fn session_key_survives_reload_preserves_comments_and_stays_out_of_exports() {
    let _guard = EnvGuard::capture();
    let directory = unique_project_temp_dir();
    let path = directory.join("config.toml");
    fs::write(&path, "# keep deployment notes\n[server]\nport = 3210\n").unwrap();
    unsafe {
        std::env::set_var(aster_gate::config::CONFIG_ENV_VAR, &path);
        std::env::remove_var("ASTER__SESSION__SIGNING_KEY");
        std::env::remove_var("ASTER__SESSION__ISSUER");
    }
    let first = aster_gate::config::load().unwrap();
    let content = fs::read_to_string(&path).unwrap();
    let second = aster_gate::config::load().unwrap();
    assert_eq!(content, fs::read_to_string(&path).unwrap());
    assert!(content.starts_with("# keep deployment notes"));
    assert_eq!(first.session.signing_key, second.session.signing_key);
    assert_ne!(first.session.signing_key, first.security.master_key);
    assert_eq!(first.session.signing_key.len(), 64);
    assert_eq!(first.session.issuer, second.session.issuer);
    assert_eq!(first.server.port, 3210);
    assert!(!format!("{first:?}").contains(&first.session.signing_key));
    assert!(
        !toml::to_string(&first)
            .unwrap()
            .contains(&first.session.signing_key)
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
}

#[test]
fn session_environment_key_preserves_numeric_hex_and_invalid_file_is_not_replaced() {
    let _guard = EnvGuard::capture();
    let directory = unique_project_temp_dir();
    let path = directory.join("config.toml");
    let key = "08".repeat(32);
    unsafe {
        std::env::set_var(aster_gate::config::CONFIG_ENV_VAR, &path);
        std::env::set_var("ASTER__SESSION__SIGNING_KEY", &key);
        std::env::set_var("ASTER__SESSION__ISSUER", "https://identity.example.test");
    }
    let loaded = aster_gate::config::load().unwrap();
    assert_eq!(loaded.session.signing_key, key);
    assert_eq!(loaded.session.issuer, "https://identity.example.test");
    assert!(!fs::read_to_string(&path).unwrap().contains(&key));
    unsafe {
        std::env::remove_var("ASTER__SESSION__SIGNING_KEY");
    }
    let invalid = "[session]\nsigning_key = 'broken'\n";
    fs::write(&path, invalid).unwrap();
    // The deployment key backfill is unrelated; isolate the session file preservation contract.
    unsafe {
        std::env::set_var("ASTER__SECURITY__MASTER_KEY", "07".repeat(32));
    }
    assert!(aster_gate::config::load().is_err());
    assert_eq!(fs::read_to_string(&path).unwrap(), invalid);
}

unsafe fn restore_env_var(key: &str, value: Option<std::ffi::OsString>) {
    match value {
        Some(value) => unsafe {
            std::env::set_var(key, value);
        },
        None => unsafe {
            std::env::remove_var(key);
        },
    }
}

fn unique_project_temp_dir() -> TestTempDir {
    let root = std::env::current_dir()
        .expect("resolve current dir")
        .join("target")
        .join("config-loader-tests");
    TestTempDir::new_in(root, "config-loader")
}

fn runtime_relative_path(path: std::path::PathBuf) -> String {
    let cwd = std::env::current_dir().expect("resolve current dir");
    path.strip_prefix(&cwd)
        .expect("test path should be under current dir")
        .to_string_lossy()
        .into_owned()
}
