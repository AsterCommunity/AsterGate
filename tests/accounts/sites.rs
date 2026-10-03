use super::*;
use aster_gate::services::system_settings::{branding::*, site::*};

#[test]
fn site_origins_are_normalized_ordered_bounded_and_unambiguous() {
    let sites = PublicSiteUrls::parse(&[
        " HTTPS://Gate.EXAMPLE.test:443/ ".into(),
        "https://panel.example.test".into(),
        "https://gate.example.test".into(),
    ])
    .unwrap();
    assert_eq!(
        sites.values(),
        ["https://gate.example.test", "https://panel.example.test"]
    );
    assert_eq!(
        sites.primary().browser_origin(),
        "https://gate.example.test"
    );
    assert!(PublicSiteUrls::parse(&[]).is_err());
    assert!(PublicSiteUrls::parse(&vec!["https://gate.example.test".into(); 17]).is_err());
    assert!(
        PublicSiteUrls::parse(
            &(0..16)
                .map(|i| format!("https://gate{i}.example.test"))
                .collect::<Vec<_>>()
        )
        .is_ok()
    );
    assert!(
        PublicSiteUrls::parse(&["http://localhost".into(), "https://localhost".into()]).is_err()
    );
    for value in [
        "*",
        "null",
        "https://*.example.test",
        "https://gate.example.test/path",
        "https://gate.example.test/?a=b",
        "https://gate.example.test/#a",
        "https://user:pass@gate.example.test",
        "http://gate.example.test",
        "javascript:alert(1)",
        "https://gate.exa\nmple.test",
        "https://gate.example.test\\evil",
    ] {
        assert!(PublicSiteOrigin::parse(value).is_err(), "{value}");
    }
    assert_eq!(
        PublicSiteOrigin::parse("http://[::1]:5173")
            .unwrap()
            .browser_origin(),
        "http://[::1]:5173"
    );
}

#[test]
fn branding_is_trimmed_byte_bounded_and_resets_to_defaults() {
    assert_eq!(BrandingTitle::parse("  Gate  ").unwrap().as_str(), "Gate");
    assert_eq!(
        BrandingTitle::parse("  ").unwrap(),
        BrandingTitle::default()
    );
    assert_eq!(
        BrandingDescription::parse("").unwrap(),
        BrandingDescription::default()
    );
    assert!(BrandingTitle::parse(&"a".repeat(120)).is_ok());
    assert!(BrandingTitle::parse(&"a".repeat(121)).is_err());
    assert!(BrandingTitle::parse(&"界".repeat(40)).is_ok());
    assert!(BrandingTitle::parse(&"界".repeat(41)).is_err());
    assert!(BrandingDescription::parse(&"a".repeat(300)).is_ok());
    assert!(BrandingDescription::parse(&"a".repeat(301)).is_err());
    for value in ["a\0b", "a\rb", "a\nb", "a\tb"] {
        assert!(BrandingTitle::parse(value).is_err());
        assert!(BrandingDescription::parse(value).is_err());
    }
}

#[tokio::test]
async fn multiple_sites_use_request_cookies_primary_mail_and_immediate_removal() {
    let f = Fixture::fresh().await;
    let mut setup = setup_input("operator");
    setup["site_urls"] = json!([
        "https://primary.example.test",
        "https://secondary.example.test",
        "http://localhost:5173"
    ]);
    setup["mail"] =
        json!({"host":"smtp.example.test", "port":587,"from_address":"accounts@example.test"});
    let response = f
        .custom_request(
            "POST",
            "/api/v1/setup",
            Some(setup),
            None,
            false,
            Some("1"),
            Some("http://localhost:5173"),
        )
        .await;
    assert_eq!(response.0, StatusCode::CREATED);
    assert!(response.2.unwrap().starts_with("ag_session="));

    let login = json!({"identifier":"operator", "password":PASSWORD});
    let secure = f
        .custom_request(
            "POST",
            "/api/v1/auth/login",
            Some(login.clone()),
            None,
            false,
            Some("1"),
            Some("https://secondary.example.test"),
        )
        .await;
    assert_eq!(secure.0, StatusCode::OK);
    let secure = secure.2.unwrap();
    assert!(secure.starts_with("__Host-ag_session=") && secure.contains("Secure"));
    assert!(!secure.contains("Domain="));
    let secure_pair = secure.split(';').next().unwrap();
    assert_eq!(
        f.custom_request(
            "GET",
            "/api/v1/account",
            None,
            Some(secure_pair),
            false,
            None,
            Some("https://secondary.example.test")
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        f.custom_request(
            "GET",
            "/api/v1/account",
            None,
            Some(secure_pair),
            false,
            None,
            Some("http://localhost:5173")
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
    let clear = f
        .custom_request(
            "POST",
            "/api/v1/auth/logout",
            None,
            Some(secure_pair),
            false,
            Some("1"),
            Some("https://secondary.example.test"),
        )
        .await;
    assert_eq!(clear.0, StatusCode::OK);
    assert!(
        clear
            .2
            .unwrap()
            .contains("__Host-ag_session=; HttpOnly; SameSite=Lax; Secure; Path=/; Max-Age=0")
    );

    for origin in [
        "null",
        "https://evil.example.test",
        "https://secondary.example.test/",
        "https://sub.secondary.example.test",
        "http://secondary.example.test",
    ] {
        assert_eq!(
            f.custom_request(
                "POST",
                "/api/v1/auth/login",
                Some(login.clone()),
                None,
                false,
                Some("1"),
                Some(origin)
            )
            .await
            .0,
            StatusCode::FORBIDDEN
        );
    }
    // Registration from a secondary site still sends mail from the first configured site.
    assert_eq!(
        f.custom_request(
            "POST",
            "/api/v1/auth/register",
            Some(registration("alice")),
            None,
            false,
            Some("1"),
            Some("https://secondary.example.test")
        )
        .await
        .0,
        StatusCode::CREATED
    );
    f.token().await;
    let first = aster_forge_mail::memory_sender_ref(&f.state.mail_sender)
        .unwrap()
        .last_message()
        .unwrap();
    assert!(
        first
            .text_body
            .contains("https://primary.example.test/verify-email#token=")
    );
    assert!(!first.text_body.contains("secondary.example.test"));

    let mut change = settings_change(1);
    change["site_urls"] = json!(["https://secondary.example.test", "http://localhost:5173"]);
    change["mail"] =
        json!({"host":"smtp.example.test", "port":587,"from_address":"accounts@example.test"});
    let update = f
        .custom_request(
            "PUT",
            "/api/v1/management/settings",
            Some({
                change["current_password"] = json!(PASSWORD);
                change
            }),
            None,
            true,
            Some("1"),
            Some("http://localhost:5173"),
        )
        .await;
    assert_eq!(update.0, StatusCode::OK, "{}", update.1);
    assert_eq!(update.1["data"]["revision"], 2);
    assert_eq!(
        f.custom_request(
            "POST",
            "/api/v1/auth/login",
            Some(login),
            None,
            false,
            Some("1"),
            Some("https://primary.example.test")
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    f.custom_request(
        "POST",
        "/api/v1/auth/register",
        Some(registration("bob")),
        None,
        false,
        Some("1"),
        Some("http://localhost:5173"),
    )
    .await;
    f.token().await;
    let next = aster_forge_mail::memory_sender_ref(&f.state.mail_sender)
        .unwrap()
        .last_message()
        .unwrap();
    assert!(
        next.text_body
            .contains("https://secondary.example.test/verify-email#token=")
    );
}

#[tokio::test]
async fn host_and_origin_spoofing_do_not_select_cookie_policy() {
    let f = Fixture::fresh().await;
    let mut setup = setup_input("operator");
    setup["site_urls"] = json!([
        "https://primary.example.test",
        "https://secondary.example.test",
        "http://localhost:5173"
    ]);
    assert_eq!(
        f.request("POST", "/api/v1/setup", Some(setup), None, false)
            .await
            .0,
        StatusCode::CREATED
    );
    for (host, origin, expected) in [
        (
            "secondary.example.test",
            Some("https://secondary.example.test"),
            StatusCode::OK,
        ),
        ("secondary.example.test", None, StatusCode::OK),
        (
            "evil.example.test",
            Some("https://secondary.example.test"),
            StatusCode::FORBIDDEN,
        ),
        (
            "primary.example.test",
            Some("https://secondary.example.test"),
            StatusCode::FORBIDDEN,
        ),
        ("evil.example.test", None, StatusCode::FORBIDDEN),
    ] {
        let mut request = Request::builder()
            .method("POST")
            .uri("/api/v1/auth/login")
            .header("Host", host)
            .header("X-AsterGate-Request", "1")
            .header("Content-Type", "application/json")
            .header("Forwarded", "host=primary.example.test;proto=https")
            .header("X-Forwarded-Host", "primary.example.test")
            .header("X-Forwarded-Proto", "https");
        if let Some(origin) = origin {
            request = request.header("Origin", origin);
        }
        let response = f
            .app
            .clone()
            .oneshot(
                request
                    .body(Body::from(
                        json!({"identifier":"operator","password":PASSWORD}).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), expected, "{host} / {origin:?}");
        if expected == StatusCode::OK {
            let cookie = response.headers()["set-cookie"].to_str().unwrap();
            assert!(
                cookie.starts_with("__Host-ag_session=")
                    && cookie.contains("Secure")
                    && !cookie.contains("Domain=")
            );
        } else {
            assert!(!response.headers().contains_key("set-cookie"));
        }
    }
}

#[tokio::test]
async fn branding_is_public_escaped_live_and_independent_of_mail_secrets() {
    let f = Fixture::fresh().await;
    let before = f
        .request("GET", "/api/v1/public/frontend-config", None, None, false)
        .await;
    assert_eq!(before.0, StatusCode::OK);
    assert_eq!(before.1["data"]["branding"]["title"], "AsterGate");
    f.request(
        "POST",
        "/api/v1/setup",
        Some(setup_input("operator")),
        None,
        false,
    )
    .await;
    let title = "<script>alert(1)</script> & {{astergate_description}}";
    let description = "\" onload=\"alert(2)\" & <img src=x>";
    let mut change = settings_change(1);
    change["branding_title"] = json!(title);
    change["branding_description"] = json!(description);
    assert_eq!(
        f.request(
            "PUT",
            "/api/v1/management/settings",
            Some(change),
            None,
            true
        )
        .await
        .0,
        StatusCode::OK
    );
    // Public rendering must never decrypt, return or depend on SMTP credentials.
    aster_forge_db::system_config::Entity::update_many()
        .col_expr(
            aster_forge_db::system_config::Column::Value,
            Expr::value("invalid-encrypted-secret"),
        )
        .filter(aster_forge_db::system_config::Column::Key.eq("mail_smtp_password"))
        .exec(f.state.db_handles.writer())
        .await
        .unwrap();
    let public = f
        .request("GET", "/api/v1/public/frontend-config", None, None, false)
        .await;
    assert_eq!(public.0, StatusCode::OK);
    assert_eq!(
        public.1["data"]["branding"],
        json!({"title":title,"description":description})
    );
    assert!(public.1["data"]["account"]["minimum_password_length"].is_number());
    assert!(!public.1.to_string().contains("invalid-encrypted-secret"));
    for path in ["/", "/sign-in", "/instance-settings"] {
        let response = f
            .app
            .clone()
            .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()["cache-control"], "no-cache");
        let bytes = to_bytes(response.into_body(), 2 * 1024 * 1024)
            .await
            .unwrap();
        let html = std::str::from_utf8(&bytes).unwrap();
        assert!(html.contains(&format!(
            "<title>{}</title>",
            aster_forge_utils::html::escape_html(title)
        )));
        assert!(html.contains(&format!(
            "content=\"{}\"",
            aster_forge_utils::html::escape_html(description)
        )));
        assert!(!html.contains("<script>alert(1)</script>"));
        assert!(!html.contains("invalid-encrypted-secret"));
    }
    let manifest = f
        .app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/manifest.webmanifest")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(manifest.headers()["cache-control"], "no-store");
    let bytes = to_bytes(manifest.into_body(), 100000).await.unwrap();
    let manifest: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(manifest["name"], title);
    assert_eq!(manifest["description"], description);
    assert_eq!(manifest["start_url"], "/");
}

#[tokio::test]
async fn invalid_and_obsolete_site_settings_fail_without_writes() {
    let f = Fixture::fresh().await;
    let mut obsolete = setup_input("operator");
    obsolete.as_object_mut().unwrap().remove("site_urls");
    obsolete["public_origin"] = json!("http://localhost:5173");
    assert_eq!(
        f.request("POST", "/api/v1/setup", Some(obsolete), None, false)
            .await
            .0,
        StatusCode::BAD_REQUEST
    );
    for sites in [
        json!([]),
        json!("https://primary.example.test"),
        json!(["https://example.test/path"]),
    ] {
        let mut input = setup_input("operator");
        input["site_urls"] = sites;
        assert_eq!(
            f.request("POST", "/api/v1/setup", Some(input), None, false)
                .await
                .0,
            StatusCode::BAD_REQUEST
        );
    }
    assert_eq!(
        account::Entity::find()
            .count(f.state.db_handles.writer())
            .await
            .unwrap(),
        0
    );
    f.request(
        "POST",
        "/api/v1/setup",
        Some(setup_input("operator")),
        None,
        false,
    )
    .await;
    for (key, invalid) in [
        ("site_urls", json!([])),
        ("branding_title", json!("a".repeat(121))),
        ("branding_description", json!("bad\nvalue")),
    ] {
        let mut input = settings_change(1);
        input[key] = invalid;
        assert_eq!(
            f.request(
                "PUT",
                "/api/v1/management/settings",
                Some(input),
                None,
                true
            )
            .await
            .0,
            StatusCode::BAD_REQUEST
        );
    }
    let current = f.state.management_settings().unwrap();
    assert_eq!(current.revision, 1);
    assert_eq!(current.branding, PublicBranding::default());
    assert_eq!(current.site_urls.values(), ["http://localhost:5173"]);
}

#[tokio::test]
async fn stale_settings_cannot_overwrite_sites_branding_or_preserved_mail_secret() {
    let f = Fixture::fresh().await;
    f.request(
        "POST",
        "/api/v1/setup",
        Some(setup_input("operator")),
        None,
        false,
    )
    .await;
    let mut fresh = settings_change(1);
    fresh["branding_title"] = json!("Updated Gate");
    fresh["mail"] = json!({"host":"smtp.example.test","port":587,"username":"sender","password":"new SMTP secret","from_address":"accounts@example.test"});
    assert_eq!(
        f.request(
            "PUT",
            "/api/v1/management/settings",
            Some(fresh),
            None,
            true
        )
        .await
        .0,
        StatusCode::OK
    );
    let stale = f
        .request(
            "PUT",
            "/api/v1/management/settings",
            Some(settings_change(1)),
            None,
            true,
        )
        .await;
    assert_eq!(stale.0, StatusCode::CONFLICT);
    assert_eq!(stale.1["code"], "settings.revision_conflict");
    assert_eq!(
        f.state
            .management_settings()
            .unwrap()
            .branding
            .title
            .as_str(),
        "Updated Gate"
    );
    assert!(f.state.management_settings().unwrap().mail.configured());
}

#[tokio::test]
async fn settings_transaction_rolls_back_all_site_values_and_revision_on_audit_failure() {
    let f = Fixture::fresh().await;
    f.request(
        "POST",
        "/api/v1/setup",
        Some(setup_input("operator")),
        None,
        false,
    )
    .await;
    let before = f.state.management_settings().unwrap();
    f.reject_inserts("audit_logs").await;
    let mut input = settings_change(1);
    input["branding_title"] = json!("Rejected Gate");
    input["site_urls"] = json!(["https://primary.example.test", "http://localhost:5173"]);
    assert_eq!(
        f.request(
            "PUT",
            "/api/v1/management/settings",
            Some(input),
            None,
            true
        )
        .await
        .0,
        StatusCode::SERVICE_UNAVAILABLE
    );
    assert_eq!(f.state.management_settings().unwrap(), before);
}

#[tokio::test]
async fn concurrent_settings_writers_on_different_accounts_have_one_revision_winner() {
    use aster_gate::{
        db::repository::system_permission_grant_repo as grants,
        services::{
            account as service, account::models::AccountId, authorization::models::SystemPermission,
        },
    };
    use sea_orm::TransactionTrait;
    let f = Fixture::fresh().await;
    f.request(
        "POST",
        "/api/v1/setup",
        Some(setup_input("operator")),
        None,
        false,
    )
    .await;
    let created = f
        .request(
            "POST",
            "/api/v1/auth/register",
            Some(registration("delegate")),
            None,
            false,
        )
        .await;
    let id = AccountId::parse(created.1["data"]["id"].as_str().unwrap()).unwrap();
    let txn = f.state.db_handles.writer().begin().await.unwrap();
    aster_gate::db::repository::system_config_repo::lock(&txn)
        .await
        .unwrap();
    grants::grant(
        &txn,
        &id,
        SystemPermission::SettingsUpdate,
        chrono::Utc::now().timestamp(),
    )
    .await
    .unwrap();
    txn.commit().await.unwrap();
    let delegate_cookie = f.login("delegate", PASSWORD).await;
    let operator_cookie = f.management_cookie.lock().unwrap().clone();
    let operator = service::authenticate(&f.state, operator_cookie.split('=').nth(1).unwrap())
        .await
        .unwrap();
    let delegate = service::authenticate(&f.state, delegate_cookie.split('=').nth(1).unwrap())
        .await
        .unwrap();
    let mut left = f.state.management_settings().unwrap();
    let mut right = left.clone();
    left.branding.title = BrandingTitle::parse("Left wins").unwrap();
    right.branding.title = BrandingTitle::parse("Right wins").unwrap();
    let (left, right) = tokio::join!(
        aster_gate::services::system_setup::update_settings(
            &f.state,
            &operator,
            left,
            Some(PASSWORD.into())
        ),
        aster_gate::services::system_setup::update_settings(
            &f.state,
            &delegate,
            right,
            Some(PASSWORD.into())
        )
    );
    assert_eq!(usize::from(left.is_ok()) + usize::from(right.is_ok()), 1);
    let (winner, loser) = if left.is_ok() {
        (left, right)
    } else {
        (right, left)
    };
    assert!(matches!(
        loser,
        Err(service::AccountError::SettingsConflict)
    ));
    assert_eq!(f.state.management_settings().unwrap(), winner.unwrap());
    assert_eq!(f.state.management_settings().unwrap().revision, 2);
}
