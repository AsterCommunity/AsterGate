use super::*;

#[tokio::test]
async fn password_change_revokes_every_session_and_challenge() {
    let f = Fixture::new().await;
    let (_, first) = f.activate("alice").await;
    let second = f.login("alice", PASSWORD).await;
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
        StatusCode::OK
    );
    let reset_token = f.token().await;
    assert_eq!(
        f.request(
            "POST",
            "/api/v1/account/password",
            Some(json!({"current_password":"wrong","new_password":NEW_PASSWORD})),
            Some(&first),
            false
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        f.request("GET", "/api/v1/account", None, Some(&second), false)
            .await
            .0,
        StatusCode::OK
    );
    assert_eq!(
        f.request(
            "POST",
            "/api/v1/account/password",
            Some(json!({"current_password":PASSWORD,"new_password":NEW_PASSWORD})),
            Some(&first),
            false
        )
        .await
        .0,
        StatusCode::OK
    );
    for cookie in [&first, &second] {
        assert_eq!(
            f.request("GET", "/api/v1/account", None, Some(cookie), false)
                .await
                .0,
            StatusCode::UNAUTHORIZED
        );
    }
    assert_eq!(
        f.request(
            "POST",
            "/api/v1/auth/reset-password",
            Some(json!({"token":reset_token,"password":NEW_PASSWORD})),
            None,
            false
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        f.request(
            "POST",
            "/api/v1/auth/login",
            Some(json!({"identifier":"alice","password":PASSWORD})),
            None,
            false
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
    f.login("alice", NEW_PASSWORD).await;
}

#[tokio::test]
async fn recovery_is_generic_single_use_and_purpose_bound() {
    let f = Fixture::new().await;
    let (_, old_cookie) = f.activate("alice").await;
    let missing = f
        .request(
            "POST",
            "/api/v1/auth/recovery",
            Some(json!({"identifier":"missing"})),
            None,
            false,
        )
        .await;
    let existing = f
        .request(
            "POST",
            "/api/v1/auth/recovery",
            Some(json!({"identifier":"alice"})),
            None,
            false,
        )
        .await;
    assert_eq!((missing.0, missing.1), (existing.0, existing.1));
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
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        f.request(
            "POST",
            "/api/v1/auth/reset-password",
            Some(json!({"token":token,"password":NEW_PASSWORD})),
            None,
            false
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        f.request("GET", "/api/v1/account", None, Some(&old_cookie), false)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        f.request(
            "POST",
            "/api/v1/auth/reset-password",
            Some(json!({"token":token,"password":PASSWORD})),
            None,
            false
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    f.login("alice", NEW_PASSWORD).await;
}

#[tokio::test]
async fn concurrent_registration_and_reset_each_have_one_winner() {
    let f = Fixture::new().await;
    let body = registration("alice");
    let (a, b) = tokio::join!(
        f.request(
            "POST",
            "/api/v1/auth/register",
            Some(body.clone()),
            None,
            false
        ),
        f.request("POST", "/api/v1/auth/register", Some(body), None, false)
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
    let token = f.token().await;
    f.request(
        "POST",
        "/api/v1/auth/verify-email",
        Some(json!({"token":token})),
        None,
        false,
    )
    .await;
    f.request(
        "POST",
        "/api/v1/auth/recovery",
        Some(json!({"identifier":"alice"})),
        None,
        false,
    )
    .await;
    let token = f.token().await;
    let body = json!({"token":token,"password":NEW_PASSWORD});
    let (a, b) = tokio::join!(
        f.request(
            "POST",
            "/api/v1/auth/reset-password",
            Some(body.clone()),
            None,
            false
        ),
        f.request(
            "POST",
            "/api/v1/auth/reset-password",
            Some(body),
            None,
            false
        )
    );
    assert_eq!(
        [a.0, b.0].iter().filter(|s| **s == StatusCode::OK).count(),
        1
    );
    assert!(
        [a.0, b.0]
            .iter()
            .all(|s| [StatusCode::OK, StatusCode::BAD_REQUEST].contains(s))
    );
    f.login("alice", NEW_PASSWORD).await;
}
