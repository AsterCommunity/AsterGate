use super::*;
use aster_forge_test::temp::SqliteTestDatabase;

use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, TransactionTrait, sea_query::Expr};

async fn fixture() -> (AppState, Principal, SqliteTestDatabase) {
    let database = SqliteTestDatabase::new("authorization-permit");
    let mut config = crate::config::AppConfig::default();
    config.database.url = database.url().into();
    config.security.master_key = hex::encode([7_u8; 32]);
    let state = crate::runtime::assembly::prepare_state(config)
        .await
        .unwrap();
    let input: crate::api::dto::system::SetupRequest = serde_json::from_value(serde_json::json!({
        "site_urls":["http://localhost:5173"], "initial_account": {
            "username":"operator", "email":"operator@example.test", "display_name":"Operator", "password":"correct horse battery staple"
        }
    })).unwrap();
    let result =
        crate::services::system_setup::initialize(&state, input.command().unwrap(), "unit test")
            .await
            .unwrap();
    let principal = crate::services::account::authenticate(&state, &result.session_token)
        .await
        .unwrap();
    (state, principal, database)
}

#[tokio::test]
async fn write_permit_rechecks_grant_after_password_confirmation() {
    let (state, principal, _database) = fixture().await;
    let permit = prepare_action(
        &state,
        &principal,
        SystemPermission::SettingsUpdate,
        Some("correct horse battery staple".into()),
    )
    .await
    .unwrap();
    let txn = state.db_handles.writer().begin().await.unwrap();
    crate::db::repository::system_config_repo::lock(&txn)
        .await
        .unwrap();
    grants::remove(&txn, &principal.account_id).await.unwrap();
    assert!(matches!(
        permit.recheck(&txn).await,
        Err(AccountError::Forbidden)
    ));
    txn.rollback().await.unwrap();
}

#[tokio::test]
async fn write_permit_rechecks_session_after_password_confirmation() {
    let (state, principal, _database) = fixture().await;
    let permit = prepare_action(
        &state,
        &principal,
        SystemPermission::AccountsDelete,
        Some("correct horse battery staple".into()),
    )
    .await
    .unwrap();
    let txn = state.db_handles.writer().begin().await.unwrap();
    crate::db::repository::system_config_repo::lock(&txn)
        .await
        .unwrap();
    account_repo::revoke_sessions(&txn, &principal.account_id, chrono::Utc::now().timestamp())
        .await
        .unwrap();
    assert!(matches!(
        permit.recheck(&txn).await,
        Err(AccountError::Unauthenticated)
    ));
    txn.rollback().await.unwrap();
}

#[tokio::test]
async fn write_permit_rejects_concurrent_revision_change_and_replayed_snapshot() {
    let (state, principal, _database) = fixture().await;
    let permit = prepare_action(&state, &principal, SystemPermission::AccountsCreate, None)
        .await
        .unwrap();
    let txn = state.db_handles.writer().begin().await.unwrap();
    crate::db::repository::system_config_repo::lock(&txn)
        .await
        .unwrap();
    assert!(
        account_repo::claim_revision(
            &txn,
            &principal.account_id,
            principal.revision,
            chrono::Utc::now().timestamp()
        )
        .await
        .unwrap()
    );
    assert!(matches!(
        permit.recheck(&txn).await,
        Err(AccountError::Conflict)
    ));
    txn.rollback().await.unwrap();
    let permit = prepare_action(&state, &principal, SystemPermission::AccountsCreate, None)
        .await
        .unwrap();
    let txn = state.db_handles.writer().begin().await.unwrap();
    crate::db::repository::system_config_repo::lock(&txn)
        .await
        .unwrap();
    permit.recheck(&txn).await.unwrap();
    txn.commit().await.unwrap();
    let permit = prepare_action(&state, &principal, SystemPermission::AccountsCreate, None)
        .await
        .unwrap();
    let txn = state.db_handles.writer().begin().await.unwrap();
    crate::db::repository::system_config_repo::lock(&txn)
        .await
        .unwrap();
    assert!(matches!(
        permit.recheck(&txn).await,
        Err(AccountError::Conflict)
    ));
    txn.rollback().await.unwrap();
}

#[tokio::test]
async fn read_check_rejects_suspended_accounts_expired_sessions_and_identity_mismatch() {
    use crate::db::entities::{account, browser_session};
    let (state, principal, _database) = fixture().await;
    let db = state.db_handles.writer();
    account::Entity::update_many()
        .col_expr(
            account::Column::Status,
            Expr::value(crate::db::entities::types::StoredAccountStatus::Suspended),
        )
        .filter(account::Column::Id.eq(principal.account_id.as_str()))
        .exec(db)
        .await
        .unwrap();
    assert!(matches!(
        require(db, &principal, SystemPermission::AccountsRead).await,
        Err(AccountError::Unauthenticated)
    ));
    account::Entity::update_many()
        .col_expr(
            account::Column::Status,
            Expr::value(crate::db::entities::types::StoredAccountStatus::Active),
        )
        .filter(account::Column::Id.eq(principal.account_id.as_str()))
        .exec(db)
        .await
        .unwrap();
    browser_session::Entity::update_many()
        .col_expr(browser_session::Column::ExpiresAt, Expr::value(0))
        .exec(db)
        .await
        .unwrap();
    assert!(matches!(
        require(db, &principal, SystemPermission::AccountsRead).await,
        Err(AccountError::Unauthenticated)
    ));
    browser_session::Entity::update_many()
        .col_expr(
            browser_session::Column::ExpiresAt,
            Expr::value(chrono::Utc::now().timestamp() + 3600),
        )
        .exec(db)
        .await
        .unwrap();
    let mut mismatched = principal;
    let id = AccountId::new();
    crate::db::repository::account_writes::create_pending(
        db,
        &id,
        crate::db::repository::account_writes::NewAccountRecord {
            display_name: "Other".into(),
            username: "other".into(),
            email: "other@example.test".into(),
            password_hash: "unused".into(),
        },
        0,
    )
    .await
    .unwrap();
    crate::db::repository::account_writes::update_status(db, &id, AccountStatus::Active, None)
        .await
        .unwrap();
    grants::grant(db, &id, SystemPermission::AccountsRead, 0)
        .await
        .unwrap();
    mismatched.account_id = id;
    assert!(matches!(
        require(db, &mismatched, SystemPermission::AccountsRead).await,
        Err(AccountError::Unauthenticated)
    ));
}
