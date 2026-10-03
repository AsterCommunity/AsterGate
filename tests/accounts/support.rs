use super::*;

pub(super) fn setup_input(username: &str) -> Value {
    json!({"site_urls":["http://localhost:5173"], "initial_account":registration(username), "registration":"open", "email_verification":"optional"})
}

pub(super) fn settings_change(revision: u64) -> Value {
    json!({"revision": revision, "site_urls": ["http://localhost:5173"], "branding_title":"AsterGate",
        "branding_description":"Account and identity management", "registration":"open", "email_verification":"optional",
        "session_ttl_seconds":86400, "challenge_ttl_seconds":1800, "mail":null})
}

pub(super) struct Fixture {
    pub(super) app: Router,
    pub(super) state: Arc<AppState>,
    _database: SqliteTestDatabase,
    _container: Option<testcontainers::ContainerAsync<testcontainers::GenericImage>>,
    pub(super) initial_account_id: String,
    pub(super) management_cookie: std::sync::Mutex<String>,
}

impl Fixture {
    pub(super) async fn new() -> Self {
        Self::with_setup(true).await
    }
    pub(super) async fn fresh() -> Self {
        Self::with_setup(false).await
    }
    pub(super) async fn with_setup(initialized: bool) -> Self {
        let database = SqliteTestDatabase::new("account-lifecycle");
        let mut config = aster_gate::config::AppConfig::default();
        config.database.url = database.url().into();
        let container = if let Ok(backend) = std::env::var("ASTER_TEST_DATABASE_BACKEND") {
            use testcontainers::{
                GenericImage, ImageExt, core::IntoContainerPort, runners::AsyncRunner,
            };
            let (image, tag, port) = match backend.as_str() {
                "postgres" => ("postgres", "16", 5432),
                "mysql" => ("mysql", "8.4", 3306),
                _ => panic!("unsupported test backend"),
            };
            let container = GenericImage::new(image, tag)
                .with_exposed_port(port.tcp())
                .with_env_var("POSTGRES_PASSWORD", "test-secret")
                .with_env_var("POSTGRES_DB", "astergate_test")
                .with_env_var("MYSQL_ROOT_PASSWORD", "test-secret")
                .with_env_var("MYSQL_DATABASE", "astergate_test")
                .start()
                .await
                .expect("start isolated account database");
            let port = container
                .get_host_port_ipv4(port.tcp())
                .await
                .expect("database port");
            let url = if backend == "postgres" {
                format!("postgres://postgres:test-secret@127.0.0.1:{port}/astergate_test")
            } else {
                format!("mysql://root:test-secret@127.0.0.1:{port}/astergate_test")
            };
            tokio::time::timeout(std::time::Duration::from_secs(90), async {
                loop {
                    if let Ok(db) = sea_orm::Database::connect(&url).await {
                        db.close().await.unwrap();
                        break;
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(250)).await;
                }
            })
            .await
            .expect("database ready");
            config.database.url = url.into();
            Some(container)
        } else {
            None
        };

        config.security.master_key = hex::encode([7_u8; 32]);
        let mut state = aster_gate::runtime::assembly::prepare_state(config)
            .await
            .expect("prepare state");
        state.mail_sender = aster_forge_mail::memory_sender();
        let (initial_account_id, management_cookie) = if initialized {
            let setup: aster_gate::api::dto::system::SetupRequest = serde_json::from_value(json!({
            "site_urls":["http://localhost:5173"],
            "initial_account":registration("instance-operator"), "registration":"open", "email_verification":"required",
            "mail":{"host":"smtp.example.test","port":587,"from_address":"accounts@example.test"}
        })).unwrap();
            let result = aster_gate::services::system_setup::initialize(
                &state,
                setup.command().unwrap(),
                "test setup",
            )
            .await
            .unwrap();
            (
                result.account.id.as_str().to_string(),
                format!("ag_session={}", result.session_token),
            )
        } else {
            (String::new(), String::new())
        };
        let state = Arc::new(state);
        Self {
            app: aster_gate::api::router(state.clone()),
            state,
            _database: database,
            _container: container,
            initial_account_id,
            management_cookie: std::sync::Mutex::new(management_cookie),
        }
    }

    pub(super) async fn request(
        &self,
        method: &str,
        path: &str,
        mut body: Option<Value>,
        cookie: Option<&str>,
        operator: bool,
    ) -> (StatusCode, Value, Option<String>) {
        if operator
            && (path.ends_with("/lifecycle") || (method == "PUT" && path.ends_with("/settings")))
            && let Some(body) = &mut body
            && body.get("current_password").is_none()
        {
            body["current_password"] = json!(PASSWORD);
        }
        self.custom_request(method, path, body, cookie, operator, Some("1"), None)
            .await
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "The router fixture exposes security headers explicitly for boundary tests."
    )]
    pub(super) async fn custom_request(
        &self,
        method: &str,
        path: &str,
        body: Option<Value>,
        cookie: Option<&str>,
        operator: bool,
        csrf: Option<&str>,
        origin: Option<&str>,
    ) -> (StatusCode, Value, Option<String>) {
        let mut request = Request::builder().method(method).uri(path);
        let management_cookie = self.management_cookie.lock().unwrap().clone();
        if let Some(cookie) = if operator {
            Some(management_cookie.as_str())
        } else {
            cookie
        } {
            request = request.header("Cookie", cookie);
        }
        if let Some(csrf) = csrf {
            request = request.header("X-AsterGate-Request", csrf);
        }
        if let Some(origin) = origin {
            request = request.header("Origin", origin);
        }
        let request = if let Some(body) = body {
            request
                .header("Content-Type", "application/json")
                .body(Body::from(body.to_string()))
        } else {
            request.body(Body::empty())
        }
        .expect("build request");
        let response = self
            .app
            .clone()
            .oneshot(request)
            .await
            .expect("router response");
        let status = response.status();
        if path.starts_with("/api/v1/account")
            || path.starts_with("/api/v1/auth")
            || path.starts_with("/api/v1/management")
        {
            assert_eq!(
                response.headers().get("cache-control").expect("no-store"),
                "no-store"
            );
        }
        let cookie = response
            .headers()
            .get("set-cookie")
            .map(|h| h.to_str().expect("cookie").to_string());
        if path == "/api/v1/setup" && status == StatusCode::CREATED {
            *self.management_cookie.lock().unwrap() = cookie
                .as_ref()
                .unwrap()
                .split(';')
                .next()
                .unwrap()
                .to_string();
        }
        let bytes = to_bytes(response.into_body(), 1024 * 1024)
            .await
            .expect("body");
        let value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        (status, value, cookie)
    }

    pub(super) async fn register(&self, username: &str) -> String {
        let (status, body, _) = self
            .request(
                "POST",
                "/api/v1/auth/register",
                Some(registration(username)),
                None,
                false,
            )
            .await;
        assert_eq!(status, StatusCode::CREATED, "{body}");
        assert_eq!(body["data"]["status"], "pending_verification");
        body["data"]["id"].as_str().expect("id").into()
    }

    pub(super) async fn token(&self) -> String {
        aster_gate::services::mail_outbox_service::dispatch_due_with(&self.state)
            .await
            .expect("dispatch queued mail");
        let message = aster_forge_mail::memory_sender_ref(&self.state.mail_sender)
            .expect("memory sender")
            .last_message()
            .expect("verification message");
        message
            .text_body
            .split("#token=")
            .nth(1)
            .expect("token link")
            .split_whitespace()
            .next()
            .expect("token")
            .into()
    }

    pub(super) async fn activate(&self, username: &str) -> (String, String) {
        let id = self.register(username).await;
        let token = self.token().await;
        assert_eq!(
            self.request(
                "POST",
                "/api/v1/auth/verify-email",
                Some(json!({"token":token})),
                None,
                false
            )
            .await
            .0,
            StatusCode::OK
        );
        (id, self.login(username, PASSWORD).await)
    }

    pub(super) async fn login(&self, username: &str, password: &str) -> String {
        let (status, body, cookie) = self
            .request(
                "POST",
                "/api/v1/auth/login",
                Some(json!({"identifier":username,"password":password})),
                None,
                false,
            )
            .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        let cookie = cookie.expect("session cookie");
        assert!(
            cookie.contains("HttpOnly")
                && cookie.contains("SameSite=Lax")
                && cookie.contains("Path=/")
        );
        assert!(!body.to_string().contains("token") && !body.to_string().contains("password"));
        cookie.split(';').next().expect("cookie pair").into()
    }

    pub(super) async fn reject_inserts(&self, table: &str) {
        assert!(["audit_logs", "mail_outbox", "browser_sessions"].contains(&table));
        let db = self.state.db_handles.writer();
        let backend = db.get_database_backend();
        let name = format!("reject_{table}");
        let statements = match backend {
            sea_orm::DatabaseBackend::Sqlite => vec![format!(
                "CREATE TRIGGER {name} BEFORE INSERT ON {table} BEGIN SELECT RAISE(ABORT, 'injected failure'); END"
            )],
            sea_orm::DatabaseBackend::MySql => vec![format!(
                "CREATE TRIGGER {name} BEFORE INSERT ON {table} FOR EACH ROW SIGNAL SQLSTATE '45000' SET MESSAGE_TEXT = 'injected failure'"
            )],
            sea_orm::DatabaseBackend::Postgres => vec![
                format!(
                    "CREATE FUNCTION {name}() RETURNS trigger LANGUAGE plpgsql AS $$BEGIN RAISE EXCEPTION 'injected failure'; END$$"
                ),
                format!(
                    "CREATE TRIGGER {name} BEFORE INSERT ON {table} FOR EACH ROW EXECUTE FUNCTION {name}()"
                ),
            ],
            _ => panic!("unsupported test database backend"),
        };
        for statement in statements {
            db.execute_unprepared(&statement).await.unwrap();
        }
    }

    pub(super) fn replace_sender(&mut self, sender: Arc<dyn aster_forge_mail::MailSender>) {
        let mut state = (*self.state).clone();
        state.mail_sender = sender;
        self.state = Arc::new(state);
        self.app = aster_gate::api::router(self.state.clone());
    }
}

pub(super) fn registration(username: &str) -> Value {
    json!({"username":username,"email":format!("{username}@example.test"),"display_name":format!("Person {username}"),"password":PASSWORD})
}
