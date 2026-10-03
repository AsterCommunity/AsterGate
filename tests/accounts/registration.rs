use super::*;

#[tokio::test]
async fn registration_activation_login_profile_and_logout() {
    let f = Fixture::new().await;
    let id = f.register("Alice").await;
    assert_eq!(
        f.request(
            "POST",
            "/api/v1/auth/login",
            Some(json!({"identifier":"Alice","password":PASSWORD})),
            None,
            false
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
    let token = f.token().await;
    let db = f.state.db_handles.writer();
    let c = challenge::Entity::find().one(db).await.unwrap().unwrap();
    assert_ne!(c.token_hash, token);
    let outbox = aster_forge_db::mail_outbox::Entity::find()
        .one(db)
        .await
        .unwrap()
        .unwrap();
    assert!(!outbox.payload_json.as_ref().contains(&token));
    assert_eq!(
        f.request(
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
    assert_eq!(
        f.request(
            "POST",
            "/api/v1/auth/verify-email",
            Some(json!({"token":token})),
            None,
            false
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    let cookie = f.login("ALICE@example.test", PASSWORD).await;
    let (status, result, _) = f
        .request(
            "PATCH",
            "/api/v1/account",
            Some(json!({"display_name":"Updated name"})),
            Some(&cookie),
            false,
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(result["data"]["display_name"], "Updated name");
    assert_eq!(result["data"]["id"], id);
    assert_eq!(result["data"]["username"], "alice");
    assert_eq!(result["data"]["email_verified"], true);
    let export = f
        .request("GET", "/api/v1/account/export", None, Some(&cookie), false)
        .await;
    assert_eq!(export.0, StatusCode::OK);
    assert!(!export.1.to_string().contains("password_hash"));
    assert_eq!(
        f.request("POST", "/api/v1/auth/logout", None, Some(&cookie), false)
            .await
            .0,
        StatusCode::OK
    );
    assert_eq!(
        f.request("GET", "/api/v1/account", None, Some(&cookie), false)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn identifier_uniqueness_invalid_input_and_unknown_fields() {
    let f = Fixture::new().await;
    f.register("alice").await;
    assert_eq!(
        f.request(
            "POST",
            "/api/v1/auth/register",
            Some(registration("ALICE")),
            None,
            false
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    for invalid in [
        json!({"password":"short"}),
        json!({"username":"bad username"}),
        json!({"email":"not-email"}),
        json!({"display_name":"\n"}),
        json!({"organization_id":"tenant-a"}),
        json!({"is_admin":true}),
    ] {
        let mut body = registration("bob");
        for (key, value) in invalid.as_object().unwrap() {
            body[key] = value.clone();
        }
        assert_eq!(
            f.request("POST", "/api/v1/auth/register", Some(body), None, false)
                .await
                .0,
            StatusCode::BAD_REQUEST
        );
    }
    assert_eq!(
        account::Entity::find()
            .filter(account::Column::Id.ne(&f.initial_account_id))
            .count(f.state.db_handles.writer())
            .await
            .unwrap(),
        1
    );
}

#[tokio::test]
async fn closed_registration_does_not_prevent_scoped_operator_provisioning() {
    let f = Fixture::new().await;
    let mut settings = f.state.management_settings().unwrap();
    settings.registration =
        aster_gate::services::system_settings::policy::RegistrationPolicy::Closed;
    let token = f
        .management_cookie
        .lock()
        .unwrap()
        .split('=')
        .nth(1)
        .unwrap()
        .to_string();
    let principal = aster_gate::services::account::authenticate(&f.state, &token)
        .await
        .unwrap();
    aster_gate::services::system_setup::update_settings(
        &f.state,
        &principal,
        settings,
        Some(PASSWORD.into()),
    )
    .await
    .unwrap();
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
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        f.request(
            "POST",
            "/api/v1/management/accounts",
            Some(registration("alice")),
            None,
            true
        )
        .await
        .0,
        StatusCode::CREATED
    );
    assert_eq!(
        f.request(
            "GET",
            "/api/v1/management/accounts?limit=0",
            None,
            None,
            true
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        f.request(
            "GET",
            "/api/v1/management/accounts?limit=101",
            None,
            None,
            true
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    let list = f
        .request(
            "GET",
            "/api/v1/management/accounts?limit=100",
            None,
            None,
            true,
        )
        .await
        .1;
    let cursor = list["data"]["items"].as_array().unwrap().last().unwrap()["id"]
        .as_str()
        .unwrap();
    let next = f
        .request(
            "GET",
            &format!("/api/v1/management/accounts?limit=1&after={cursor}"),
            None,
            None,
            true,
        )
        .await
        .1;
    assert_eq!(next["data"]["items"].as_array().unwrap().len(), 0);
}
