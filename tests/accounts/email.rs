use super::*;

#[tokio::test]
async fn resend_expiry_pending_restore_and_stale_links() {
    let f = Fixture::new().await;
    let id = f.register("alice").await;
    let old = f.token().await;
    f.request(
        "POST",
        "/api/v1/auth/verification",
        Some(json!({"identifier":"alice"})),
        None,
        false,
    )
    .await;
    let new = f.token().await;
    assert_ne!(old, new);
    assert_eq!(
        f.request(
            "POST",
            "/api/v1/auth/verify-email",
            Some(json!({"token":old})),
            None,
            false
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    let path = format!("/api/v1/management/accounts/{id}/lifecycle");
    f.request("POST", &path, Some(json!({"action":"suspend"})), None, true)
        .await;
    let restore = f
        .request("POST", &path, Some(json!({"action":"restore"})), None, true)
        .await;
    assert_eq!(restore.1["data"]["status"], "pending_verification");
    assert_eq!(
        f.request(
            "POST",
            "/api/v1/auth/verify-email",
            Some(json!({"token":new})),
            None,
            false
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    f.request(
        "POST",
        "/api/v1/auth/verification",
        Some(json!({"identifier":"alice"})),
        None,
        false,
    )
    .await;
    let expiring = f.token().await;
    challenge::Entity::update_many()
        .col_expr(challenge::Column::ExpiresAt, Expr::value(0))
        .exec(f.state.db_handles.writer())
        .await
        .unwrap();
    assert_eq!(
        f.request(
            "POST",
            "/api/v1/auth/verify-email",
            Some(json!({"token":expiring})),
            None,
            false
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
}

#[tokio::test]
async fn concurrent_verification_has_one_winner_and_no_duplicate_login() {
    let f = Fixture::new().await;
    f.register("alice").await;
    let token = f.token().await;
    let body = json!({"token":token});
    let (a, b) = tokio::join!(
        f.request(
            "POST",
            "/api/v1/auth/verify-email",
            Some(body.clone()),
            None,
            false
        ),
        f.request("POST", "/api/v1/auth/verify-email", Some(body), None, false)
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
}

#[tokio::test]
async fn challenge_consumption_rolls_back_when_audit_fails() {
    let f = Fixture::new().await;
    let id = f.register("alice").await;
    let token = f.token().await;
    f.reject_inserts("audit_logs").await;
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
        StatusCode::SERVICE_UNAVAILABLE
    );
    let db = f.state.db_handles.writer();
    assert_eq!(
        account::Entity::find_by_id(id)
            .one(db)
            .await
            .unwrap()
            .unwrap()
            .status,
        aster_gate::db::entities::types::StoredAccountStatus::PendingVerification
    );
    assert!(
        challenge::Entity::find()
            .one(db)
            .await
            .unwrap()
            .unwrap()
            .consumed_at
            .is_none()
    );
    assert!(
        identifier::Entity::find()
            .all(db)
            .await
            .unwrap()
            .iter()
            .all(|i| i.verified_at.is_none())
    );
}
