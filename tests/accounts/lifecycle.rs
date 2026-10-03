use super::*;

#[tokio::test]
async fn suspension_restore_and_terminal_deletion() {
    let f = Fixture::new().await;
    let (id, cookie) = f.activate("alice").await;
    let path = format!("/api/v1/management/accounts/{id}/lifecycle");
    assert_eq!(
        f.request("POST", &path, Some(json!({"action":"restore"})), None, true)
            .await
            .0,
        StatusCode::CONFLICT
    );
    assert_eq!(
        f.request("POST", &path, Some(json!({"action":"suspend"})), None, true)
            .await
            .0,
        StatusCode::OK
    );
    assert_eq!(
        f.request("POST", &path, Some(json!({"action":"suspend"})), None, true)
            .await
            .0,
        StatusCode::CONFLICT
    );
    assert_eq!(
        f.request("GET", "/api/v1/account", None, Some(&cookie), false)
            .await
            .0,
        StatusCode::UNAUTHORIZED
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
    assert_eq!(
        f.request("POST", &path, Some(json!({"action":"restore"})), None, true)
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
    let new_cookie = f.login("alice", PASSWORD).await;
    assert_eq!(
        f.request(
            "DELETE",
            "/api/v1/account",
            Some(json!({"password":"wrong"})),
            Some(&new_cookie),
            false
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        f.request(
            "DELETE",
            "/api/v1/account",
            Some(json!({"password":PASSWORD})),
            Some(&new_cookie),
            false
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        f.request("POST", &path, Some(json!({"action":"restore"})), None, true)
            .await
            .0,
        StatusCode::CONFLICT
    );
    let db = f.state.db_handles.writer();
    let row = account::Entity::find_by_id(&id)
        .one(db)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        row.status,
        aster_gate::db::entities::types::StoredAccountStatus::Deleted
    );
    assert!(row.display_name.is_empty());
    assert_eq!(
        identifier::Entity::find()
            .filter(identifier::Column::AccountId.ne(&f.initial_account_id))
            .count(db)
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        credential::Entity::find()
            .filter(credential::Column::AccountId.ne(&f.initial_account_id))
            .count(db)
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        session::Entity::find()
            .filter(session::Column::AccountId.ne(&f.initial_account_id))
            .count(db)
            .await
            .unwrap(),
        0
    );
    assert_eq!(challenge::Entity::find().count(db).await.unwrap(), 0);
    let mail = aster_forge_db::mail_outbox::Entity::find()
        .all(db)
        .await
        .unwrap();
    assert!(
        mail.iter()
            .all(|m| m.to_address.is_empty() && m.payload_json.as_ref() == "{}")
    );
    let new_id = f.register("alice").await;
    assert_ne!(new_id, id);
}

#[tokio::test]
async fn audit_failure_rolls_back_credentials_sessions_and_status() {
    let f = Fixture::new().await;
    let (id, cookie) = f.activate("alice").await;
    let db = f.state.db_handles.writer();
    let backend = db.get_database_backend();
    let statements = match backend {
        sea_orm::DatabaseBackend::Sqlite => vec![
            "CREATE TRIGGER reject_account_audit BEFORE INSERT ON audit_logs BEGIN SELECT RAISE(ABORT, 'injected failure'); END",
        ],
        sea_orm::DatabaseBackend::MySql => vec![
            "CREATE TRIGGER reject_account_audit BEFORE INSERT ON audit_logs FOR EACH ROW SIGNAL SQLSTATE '45000' SET MESSAGE_TEXT = 'injected failure'",
        ],
        sea_orm::DatabaseBackend::Postgres => vec![
            "CREATE FUNCTION reject_account_audit() RETURNS trigger LANGUAGE plpgsql AS $$BEGIN RAISE EXCEPTION 'injected failure'; END$$",
            "CREATE TRIGGER reject_account_audit BEFORE INSERT ON audit_logs FOR EACH ROW EXECUTE FUNCTION reject_account_audit()",
        ],
        _ => panic!("unsupported test database backend"),
    };
    for statement in statements {
        db.execute_unprepared(statement).await.unwrap();
    }
    assert_eq!(
        f.request(
            "POST",
            "/api/v1/account/password",
            Some(json!({"current_password":PASSWORD,"new_password":NEW_PASSWORD})),
            Some(&cookie),
            false
        )
        .await
        .0,
        StatusCode::SERVICE_UNAVAILABLE
    );
    assert_eq!(
        f.request("GET", "/api/v1/account", None, Some(&cookie), false)
            .await
            .0,
        StatusCode::OK
    );
    let credential = credential::Entity::find_by_id(&id)
        .one(db)
        .await
        .unwrap()
        .unwrap();
    assert!(aster_forge_crypto::verify_password(PASSWORD, &credential.password_hash).unwrap());
    assert_eq!(
        f.request(
            "POST",
            &format!("/api/v1/management/accounts/{id}/lifecycle"),
            Some(json!({"action":"suspend"})),
            None,
            true
        )
        .await
        .0,
        StatusCode::SERVICE_UNAVAILABLE
    );
    assert_eq!(
        account::Entity::find_by_id(&id)
            .one(db)
            .await
            .unwrap()
            .unwrap()
            .status,
        aster_gate::db::entities::types::StoredAccountStatus::Active
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
        .0,
        StatusCode::SERVICE_UNAVAILABLE
    );
    assert_eq!(
        identifier::Entity::find()
            .filter(identifier::Column::AccountId.ne(&f.initial_account_id))
            .count(db)
            .await
            .unwrap(),
        2
    );
}
