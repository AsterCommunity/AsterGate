use super::*;

#[tokio::test]
async fn self_service_ownership_and_operator_boundaries() {
    let f = Fixture::new().await;
    let (_, alice) = f.activate("alice").await;
    let (bob_id, bob) = f.activate("bob").await;
    let bob_sessions = f
        .request("GET", "/api/v1/account/sessions", None, Some(&bob), false)
        .await
        .1;
    let bob_session = bob_sessions["data"][0]["id"].as_str().unwrap();
    assert_eq!(
        f.request(
            "DELETE",
            &format!("/api/v1/account/sessions/{bob_session}"),
            None,
            Some(&alice),
            false
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        f.request("GET", "/api/v1/account", None, Some(&bob), false)
            .await
            .0,
        StatusCode::OK
    );
    assert_eq!(
        f.request(
            "GET",
            "/api/v1/management/accounts",
            None,
            Some(&alice),
            false
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        f.request(
            "POST",
            &format!("/api/v1/management/accounts/{bob_id}/lifecycle"),
            Some(json!({"action":"suspend"})),
            Some(&alice),
            false
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        f.request("GET", "/api/v1/account", None, None, true)
            .await
            .0,
        StatusCode::OK
    );
    assert_eq!(
        f.request(
            "GET",
            "/api/v1/management/accounts?organization_id=tenant-a",
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
            "PATCH",
            "/api/v1/account",
            Some(json!({"display_name":"attack","account_id":bob_id})),
            Some(&alice),
            false
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
}

#[tokio::test]
async fn csrf_origin_cookie_ambiguity_and_malformed_bodies() {
    let f = Fixture::new().await;
    assert_eq!(
        f.custom_request(
            "POST",
            "/api/v1/auth/register",
            Some(registration("alice")),
            None,
            false,
            None,
            None
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        f.custom_request(
            "POST",
            "/api/v1/auth/register",
            Some(registration("alice")),
            None,
            false,
            Some("1"),
            Some("https://evil.test")
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    let (_, cookie) = f.activate("alice").await;
    let duplicate = format!("{cookie}; {cookie}");
    assert_eq!(
        f.request("GET", "/api/v1/account", None, Some(&duplicate), false)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        f.custom_request(
            "POST",
            "/api/v1/auth/logout",
            None,
            Some(&cookie),
            false,
            Some("1"),
            Some("http://localhost:5173")
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        f.request("POST", "/api/v1/auth/login", None, None, false)
            .await
            .0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        f.request(
            "POST",
            "/api/v1/auth/login",
            Some(json!({"identifier":"alice","password":"x".repeat(20000)})),
            None,
            false
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
}

#[tokio::test]
async fn login_reservations_survive_restart_and_session_expiry_is_enforced() {
    let f = Fixture::new().await;
    let (_, cookie) = f.activate("alice").await;
    session::Entity::update_many()
        .col_expr(session::Column::ExpiresAt, Expr::value(0))
        .exec(f.state.db_handles.writer())
        .await
        .unwrap();
    assert_eq!(
        f.request("GET", "/api/v1/account", None, Some(&cookie), false)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    for _ in 0..20 {
        let _ = f
            .request(
                "POST",
                "/api/v1/auth/login",
                Some(json!({"identifier":"ghost","password":"wrong"})),
                None,
                false,
            )
            .await;
    }
    let result = f
        .request(
            "POST",
            "/api/v1/auth/login",
            Some(json!({"identifier":"ghost","password":"wrong"})),
            None,
            false,
        )
        .await;
    assert_eq!(result.0, StatusCode::TOO_MANY_REQUESTS);
    let budgets = aster_gate::db::entities::authentication_budget::Entity::find()
        .all(f.state.db_handles.writer())
        .await
        .unwrap();
    assert!(budgets.iter().all(|b| !b.key.contains("ghost")));
    let fresh = aster_gate::api::router(f.state.clone());
    let response = fresh
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/login")
                .header("X-AsterGate-Request", "1")
                .header("Content-Type", "application/json")
                .body(Body::from(
                    json!({"identifier":"ghost","password":"wrong"}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
}

#[tokio::test]
async fn session_insert_failure_rolls_back_login_and_revision() {
    let f = Fixture::new().await;
    let (id, cookie) = f.activate("alice").await;
    let db = f.state.db_handles.writer();
    let original = account::Entity::find_by_id(&id)
        .one(db)
        .await
        .unwrap()
        .unwrap();
    f.reject_inserts("browser_sessions").await;
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
        StatusCode::SERVICE_UNAVAILABLE
    );
    assert_eq!(
        account::Entity::find_by_id(id)
            .one(db)
            .await
            .unwrap()
            .unwrap()
            .revision,
        original.revision
    );
    assert_eq!(
        f.request("GET", "/api/v1/account", None, Some(&cookie), false)
            .await
            .0,
        StatusCode::OK
    );
}
