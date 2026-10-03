use super::*;

#[tokio::test]
async fn setup_without_smtp_allows_unverified_registration_and_closes_permanently() {
    let f = Fixture::fresh().await;
    assert_eq!(
        f.request("GET", "/api/v1/public/frontend-config", None, None, false)
            .await
            .1["data"]["account"]["setup_state"],
        "needs_setup"
    );
    assert_eq!(
        f.request(
            "POST",
            "/api/v1/auth/register",
            Some(registration("alice")),
            None,
            false
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    let (status, body, cookie) = f
        .request(
            "POST",
            "/api/v1/setup",
            Some(setup_input("operator")),
            None,
            false,
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert_eq!(body["data"]["email_verified"], false);
    assert_eq!(body["data"]["status"], "active");
    assert_eq!(
        body["data"]["system_permissions"].as_array().unwrap().len(),
        7
    );
    let cookie = cookie.unwrap().split(';').next().unwrap().to_string();
    assert_eq!(
        f.request(
            "GET",
            "/api/v1/management/accounts",
            None,
            Some(&cookie),
            false
        )
        .await
        .0,
        StatusCode::OK
    );
    let (status, body, _) = f
        .request(
            "POST",
            "/api/v1/auth/register",
            Some(registration("alice")),
            None,
            false,
        )
        .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(body["data"]["status"], "active");
    assert_eq!(body["data"]["email_verified"], false);
    f.login("alice", PASSWORD).await;
    assert_eq!(
        f.request(
            "POST",
            "/api/v1/auth/recovery",
            Some(json!({"identifier":"alice"})),
            None,
            false
        )
        .await
        .0,
        StatusCode::SERVICE_UNAVAILABLE
    );
    assert_eq!(
        f.request(
            "POST",
            "/api/v1/setup",
            Some(setup_input("another")),
            None,
            false
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    assert_eq!(
        f.request("GET", "/api/v1/setup/status", None, None, false)
            .await
            .1["data"],
        "ready"
    );
    assert_eq!(
        aster_forge_db::mail_outbox::Entity::find()
            .count(f.state.db_handles.writer())
            .await
            .unwrap(),
        0
    );
}

#[tokio::test]
async fn setup_checks_origin_and_concurrent_transaction_claim() {
    let f = Fixture::fresh().await;
    assert_eq!(
        f.custom_request(
            "POST",
            "/api/v1/setup",
            Some(setup_input("operator")),
            None,
            false,
            Some("1"),
            Some("https://evil.test")
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    let (a, b) = tokio::join!(
        f.request(
            "POST",
            "/api/v1/setup",
            Some(setup_input("operator-a")),
            None,
            false
        ),
        f.request(
            "POST",
            "/api/v1/setup",
            Some(setup_input("operator-b")),
            None,
            false
        )
    );
    assert_eq!(
        [a.0, b.0]
            .iter()
            .filter(|s| **s == StatusCode::CREATED)
            .count(),
        1
    );
    assert!(
        [a.0, b.0]
            .iter()
            .all(|s| [StatusCode::CREATED, StatusCode::CONFLICT].contains(s))
    );
    assert_eq!(
        account::Entity::find()
            .count(f.state.db_handles.writer())
            .await
            .unwrap(),
        1
    );
}

#[tokio::test]
async fn setup_audit_failure_rolls_back_settings_credentials_and_completion() {
    let f = Fixture::fresh().await;
    f.reject_inserts("audit_logs").await;
    assert_eq!(
        f.request(
            "POST",
            "/api/v1/setup",
            Some(setup_input("operator")),
            None,
            false
        )
        .await
        .0,
        StatusCode::SERVICE_UNAVAILABLE
    );
    assert_eq!(
        f.request("GET", "/api/v1/setup/status", None, None, false)
            .await
            .1["data"],
        "needs_setup"
    );
    assert_eq!(
        account::Entity::find()
            .count(f.state.db_handles.writer())
            .await
            .unwrap(),
        0
    );
    let row = aster_forge_db::system_config::Entity::find()
        .filter(aster_forge_db::system_config::Column::Key.eq("public_site_urls"))
        .one(f.state.db_handles.writer())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.value, "[]");
}

#[tokio::test]
async fn verification_required_needs_smtp_and_corrupt_bootstrap_never_reopens() {
    let f = Fixture::fresh().await;
    let mut input = setup_input("operator");
    input["email_verification"] = json!("required");
    assert_eq!(
        f.request("POST", "/api/v1/setup", Some(input), None, false)
            .await
            .0,
        StatusCode::SERVICE_UNAVAILABLE
    );
    f.request(
        "POST",
        "/api/v1/setup",
        Some(setup_input("operator")),
        None,
        false,
    )
    .await;
    aster_forge_db::system_config::Entity::update_many()
        .col_expr(
            aster_forge_db::system_config::Column::Value,
            Expr::value("false"),
        )
        .filter(aster_forge_db::system_config::Column::Key.eq("setup_completed"))
        .exec(f.state.db_handles.writer())
        .await
        .unwrap();
    assert_eq!(
        f.request("GET", "/api/v1/setup/status", None, None, false)
            .await
            .0,
        StatusCode::SERVICE_UNAVAILABLE
    );
    assert_eq!(
        f.request(
            "POST",
            "/api/v1/setup",
            Some(setup_input("another")),
            None,
            false
        )
        .await
        .0,
        StatusCode::SERVICE_UNAVAILABLE
    );
}
