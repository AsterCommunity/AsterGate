use super::*;

#[tokio::test]
async fn last_operator_protection_and_settings_permissions() {
    let f = Fixture::fresh().await;
    let (_, body, cookie) = f
        .request(
            "POST",
            "/api/v1/setup",
            Some(setup_input("operator")),
            None,
            false,
        )
        .await;
    let id = body["data"]["id"].as_str().unwrap();
    let cookie = cookie.unwrap().split(';').next().unwrap().to_string();
    let path = format!("/api/v1/management/accounts/{id}/lifecycle");
    assert_eq!(
        f.request(
            "POST",
            &path,
            Some(json!({"action":"suspend","current_password":PASSWORD})),
            Some(&cookie),
            false
        )
        .await
        .1["code"],
        "account.last_operator"
    );
    assert_eq!(
        f.request(
            "DELETE",
            "/api/v1/account",
            Some(json!({"password":PASSWORD})),
            Some(&cookie),
            false
        )
        .await
        .1["code"],
        "account.last_operator"
    );
    let ordinary = f
        .request(
            "POST",
            "/api/v1/auth/register",
            Some(registration("alice")),
            None,
            false,
        )
        .await;
    assert_eq!(ordinary.0, StatusCode::CREATED);
    let alice = f.login("alice", PASSWORD).await;
    assert_eq!(
        f.request(
            "GET",
            "/api/v1/management/settings",
            None,
            Some(&alice),
            false
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    let settings = f
        .request(
            "GET",
            "/api/v1/management/settings",
            None,
            Some(&cookie),
            false,
        )
        .await;
    assert_eq!(settings.0, StatusCode::OK);
    assert!(settings.1["data"]["mail"].is_null());
    let mut change = settings_change(1);
    change["current_password"] = json!("wrong");
    assert_eq!(
        f.request(
            "PUT",
            "/api/v1/management/settings",
            Some(change.clone()),
            Some(&cookie),
            false
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
    change["current_password"] = json!(PASSWORD);
    assert_eq!(
        f.request(
            "PUT",
            "/api/v1/management/settings",
            Some(change),
            Some(&cookie),
            false
        )
        .await
        .0,
        StatusCode::OK
    );
}

#[tokio::test]
async fn smtp_can_be_added_after_setup_without_trusting_unverified_email_for_recovery() {
    let f = Fixture::fresh().await;
    f.request(
        "POST",
        "/api/v1/setup",
        Some(setup_input("operator")),
        None,
        false,
    )
    .await;
    f.request(
        "POST",
        "/api/v1/auth/register",
        Some(registration("alice")),
        None,
        false,
    )
    .await;
    let mail = json!({"host":"smtp.example.test","port":587,"username":"sender","password":"SMTP-password","from_address":"accounts@example.test"});
    let mut change = settings_change(1);
    change["mail"] = mail;
    let response = f
        .request(
            "PUT",
            "/api/v1/management/settings",
            Some(change.clone()),
            None,
            true,
        )
        .await;
    assert_eq!(response.0, StatusCode::OK);
    assert_eq!(response.1["data"]["mail"]["has_password"], true);
    assert!(!response.1.to_string().contains("SMTP-password"));
    let db = f.state.db_handles.writer();
    let row = aster_forge_db::system_config::Entity::find()
        .filter(aster_forge_db::system_config::Column::Key.eq("mail_smtp_password"))
        .one(db)
        .await
        .unwrap()
        .unwrap();
    assert!(row.is_sensitive);
    assert!(!row.value.contains("SMTP-password"));
    assert!(row.value.starts_with("v1:"));
    f.request(
        "POST",
        "/api/v1/auth/recovery",
        Some(json!({"identifier":"alice"})),
        None,
        false,
    )
    .await;
    assert_eq!(challenge::Entity::find().count(db).await.unwrap(), 0);
    f.request(
        "POST",
        "/api/v1/auth/verification",
        Some(json!({"identifier":"alice"})),
        None,
        false,
    )
    .await;
    let token = f.token().await;
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
    f.request(
        "POST",
        "/api/v1/auth/recovery",
        Some(json!({"identifier":"alice"})),
        None,
        false,
    )
    .await;
    assert!(challenge::Entity::find().count(db).await.unwrap() >= 1);
    let mut preserve = change;
    preserve["revision"] = json!(2);
    preserve["mail"].as_object_mut().unwrap().remove("password");
    assert_eq!(
        f.request(
            "PUT",
            "/api/v1/management/settings",
            Some(preserve),
            None,
            true
        )
        .await
        .1["data"]["mail"]["has_password"],
        true
    );
}
