use super::*;
use aster_gate::{
    api::dto::authorization::SystemPermissionDto,
    db::{
        entities::{system_permission_grant as grant, types::SystemPermissionValue},
        repository::system_permission_grant_repo,
    },
    services::{
        account::{self as account_service, AccountError, models::AccountId},
        authorization::{self, models::*},
    },
};
use sea_orm::{ActiveEnum, TransactionTrait};

async fn assign(f: &Fixture, id: &str, permissions: &[SystemPermission]) {
    let txn = f.state.db_handles.writer().begin().await.unwrap();
    aster_gate::db::repository::system_config_repo::lock(&txn)
        .await
        .unwrap();
    let id = AccountId::parse(id).unwrap();
    system_permission_grant_repo::remove(&txn, &id)
        .await
        .unwrap();
    for &permission in permissions {
        system_permission_grant_repo::grant(&txn, &id, permission, chrono::Utc::now().timestamp())
            .await
            .unwrap();
    }
    txn.commit().await.unwrap();
}

async fn principal(f: &Fixture, cookie: &str) -> account_service::models::Principal {
    account_service::authenticate(&f.state, cookie.split('=').nth(1).unwrap())
        .await
        .unwrap()
}

fn settings_input() -> Value {
    let mut value = settings_change(1);
    value["registration"] = json!("closed");
    value["current_password"] = json!(PASSWORD);
    value
}

#[test]
fn registry_wire_and_storage_have_identical_unique_ids() {
    let mut ids = std::collections::HashSet::new();
    assert_eq!(SystemPermission::ALL.len(), 7);
    for permission in SystemPermission::ALL {
        let definition = permission.definition();
        assert_eq!(definition.permission, permission);
        assert!(ids.insert(permission.as_str()));
        assert_eq!(
            SystemPermission::parse(permission.as_str()),
            Some(permission)
        );
        assert_eq!(
            permission.as_str(),
            format!(
                "{}.{}.{}",
                SystemPermission::NAMESPACE,
                definition.resource.as_str(),
                definition.action.as_str()
            )
        );
        assert_eq!(
            PermissionKey::from(permission).as_str(),
            permission.as_str()
        );
        let stored = SystemPermissionValue::from(permission);
        assert_eq!(stored.to_value(), permission.as_str());
        assert_eq!(SystemPermission::from(stored), permission);
        let wire = serde_json::to_value(SystemPermissionDto::from(permission)).unwrap();
        assert_eq!(wire, permission.as_str());
        assert_eq!(
            serde_json::from_value::<SystemPermissionDto>(wire).unwrap(),
            SystemPermissionDto::from(permission)
        );
        assert_eq!(
            definition.protects_last_holder,
            matches!(
                permission,
                SystemPermission::AccountsCreate | SystemPermission::SettingsUpdate
            )
        );
        assert_eq!(
            definition.authentication == AuthenticationRequirement::CurrentPassword,
            matches!(
                permission,
                SystemPermission::AccountsSuspend
                    | SystemPermission::AccountsRestore
                    | SystemPermission::AccountsDelete
                    | SystemPermission::SettingsUpdate
            )
        );
    }
}

#[test]
fn permission_keys_reject_wildcards_scope_spoofing_and_unregistered_product_names() {
    for invalid in [
        "",
        "manage_accounts",
        "astergate.system.accounts.manage",
        "astergate.system.*",
        "astergate.system.accounts.*",
        "astergate.system",
        "astergate.org.accounts.delete",
        "astergate.application.accounts.delete",
        "astergate.system.accounts.delete.extra",
        "ASTERGATE.system.accounts.read",
        " astergate.system.accounts.read",
        "astergate.system.accounts.read ",
        "astergate..accounts.read",
        ".files.read",
        "files.read.",
        "files..read",
        "files/read",
        "files.read\n",
        "fílés.read",
        "files.1read",
        "files.%72ead",
        "files.read*",
    ] {
        assert!(
            PermissionKey::parse(invalid).is_err(),
            "accepted {invalid:?}"
        );
        assert!(SystemPermission::parse(invalid).is_none());
        assert!(serde_json::from_value::<SystemPermissionDto>(json!(invalid)).is_err());
    }
    for custom in [
        "files.read",
        "acme.invoice.approve",
        "acme2.documents.read_own",
    ] {
        assert_eq!(PermissionKey::parse(custom).unwrap().as_str(), custom);
        assert!(SystemPermission::parse(custom).is_none());
    }
    let boundary = format!("app.{}", "a".repeat(PermissionKey::MAX_LEN - 4));
    assert!(PermissionKey::parse(&boundary).is_ok());
    assert!(PermissionKey::parse(&(boundary + "a")).is_err());
}

#[tokio::test]
async fn every_permission_authorizes_only_its_operation_through_real_router() {
    for permission in SystemPermission::ALL {
        let f = Fixture::new().await;
        let (actor_id, cookie) = f.activate("delegate").await;
        let (target_id, _) = f.activate("target").await;
        assign(&f, &actor_id, &[permission]).await;
        let (status, body, _) = f
            .request("GET", "/api/v1/account", None, Some(&cookie), false)
            .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            body["data"]["system_permissions"],
            json!([permission.as_str()])
        );
        let expected = |operation, ok| {
            if operation == permission {
                ok
            } else {
                StatusCode::FORBIDDEN
            }
        };
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
            expected(SystemPermission::AccountsRead, StatusCode::OK)
        );
        assert_eq!(
            f.request(
                "POST",
                "/api/v1/management/accounts",
                Some(registration("provisioned")),
                Some(&cookie),
                false
            )
            .await
            .0,
            expected(SystemPermission::AccountsCreate, StatusCode::CREATED)
        );
        assert_eq!(
            f.request(
                "GET",
                "/api/v1/management/settings",
                None,
                Some(&cookie),
                false
            )
            .await
            .0,
            expected(SystemPermission::SettingsRead, StatusCode::OK)
        );
        assert_eq!(
            f.request(
                "PUT",
                "/api/v1/management/settings",
                Some(settings_input()),
                Some(&cookie),
                false
            )
            .await
            .0,
            expected(SystemPermission::SettingsUpdate, StatusCode::OK)
        );
        let path = format!("/api/v1/management/accounts/{target_id}/lifecycle");
        for (action, required) in [
            ("suspend", SystemPermission::AccountsSuspend),
            ("restore", SystemPermission::AccountsRestore),
            ("delete", SystemPermission::AccountsDelete),
        ] {
            if action == "restore" {
                // The full operator establishes a valid target state so the narrow grant is tested on its success path.
                let db = f.state.db_handles.writer();
                let status = account::Entity::find_by_id(&target_id)
                    .one(db)
                    .await
                    .unwrap()
                    .unwrap()
                    .status;
                if status != aster_gate::db::entities::types::StoredAccountStatus::Suspended {
                    assert_eq!(
                        f.request("POST", &path, Some(json!({"action":"suspend"})), None, true)
                            .await
                            .0,
                        StatusCode::OK
                    );
                }
            }
            let result = f
                .request(
                    "POST",
                    &path,
                    Some(json!({"action":action,"current_password":PASSWORD})),
                    Some(&cookie),
                    false,
                )
                .await;
            assert_eq!(
                result.0,
                expected(required, StatusCode::OK),
                "{permission:?} {action}: {}",
                result.1
            );
        }
    }
}

#[tokio::test]
async fn forbidden_mutations_have_no_effect_and_cannot_self_grant() {
    let f = Fixture::new().await;
    let (actor_id, cookie) = f.activate("reader").await;
    let (target_id, _) = f.activate("target").await;
    assign(
        &f,
        &actor_id,
        &[
            SystemPermission::AccountsRead,
            SystemPermission::SettingsRead,
        ],
    )
    .await;
    let db = f.state.db_handles.writer();
    let before = account::Entity::find_by_id(&target_id)
        .one(db)
        .await
        .unwrap()
        .unwrap();
    let audit_count = aster_forge_db::audit_log::Entity::find()
        .count(db)
        .await
        .unwrap();
    for action in ["suspend", "restore", "delete"] {
        assert_eq!(
            f.request(
                "POST",
                &format!("/api/v1/management/accounts/{target_id}/lifecycle"),
                Some(json!({"action":action,"current_password":PASSWORD})),
                Some(&cookie),
                false
            )
            .await
            .0,
            StatusCode::FORBIDDEN
        );
    }
    assert_eq!(
        account::Entity::find_by_id(&target_id)
            .one(db)
            .await
            .unwrap()
            .unwrap(),
        before
    );
    assert_eq!(
        aster_forge_db::audit_log::Entity::find()
            .count(db)
            .await
            .unwrap(),
        audit_count
    );
    assert_eq!(f.request("PATCH", "/api/v1/account", Some(json!({"display_name":"attack","system_permissions":[SystemPermission::AccountsDelete.as_str()]})), Some(&cookie), false).await.0, StatusCode::BAD_REQUEST);
    assert_eq!(
        f.request(
            "GET",
            "/api/v1/management/accounts?organization_id=other",
            None,
            Some(&cookie),
            false
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    let grants = system_permission_grant_repo::list(db, &AccountId::parse(&actor_id).unwrap())
        .await
        .unwrap();
    assert!(!grants.contains(&SystemPermission::AccountsDelete));
}

#[tokio::test]
async fn all_high_risk_permissions_require_current_password_in_addition_to_grant() {
    let f = Fixture::new().await;
    let (id, cookie) = f.activate("target").await;
    let (actor_id, actor) = f.activate("delegate").await;
    assign(&f, &actor_id, &SystemPermission::ALL).await;
    for permission in [
        SystemPermission::AccountsSuspend,
        SystemPermission::AccountsRestore,
        SystemPermission::AccountsDelete,
        SystemPermission::SettingsUpdate,
    ] {
        for password in [
            None,
            Some("incorrect"),
            Some("correct horse battery staple "),
        ] {
            let (path, mut body) = if permission == SystemPermission::SettingsUpdate {
                let mut body = settings_input();
                body.as_object_mut().unwrap().remove("current_password");
                ("/api/v1/management/settings".to_string(), body)
            } else {
                let action = permission.definition().action.as_str();
                (
                    format!("/api/v1/management/accounts/{id}/lifecycle"),
                    json!({"action":action}),
                )
            };
            if let Some(password) = password {
                body["current_password"] = json!(password);
            }
            assert_eq!(
                f.request(
                    if permission == SystemPermission::SettingsUpdate {
                        "PUT"
                    } else {
                        "POST"
                    },
                    &path,
                    Some(body),
                    Some(&actor),
                    false
                )
                .await
                .0,
                StatusCode::UNAUTHORIZED
            );
        }
    }
    assert_eq!(
        f.request("GET", "/api/v1/account", None, Some(&cookie), false)
            .await
            .0,
        StatusCode::OK
    );
}

#[tokio::test]
async fn stale_principals_cannot_use_revoked_permissions_or_sessions() {
    let f = Fixture::new().await;
    let (id, cookie) = f.activate("delegate").await;
    assign(&f, &id, &[SystemPermission::AccountsRead]).await;
    let snapshot = principal(&f, &cookie).await;
    assert!(
        account_service::manage_list(&f.state, &snapshot, None, 10)
            .await
            .is_ok()
    );
    assign(&f, &id, &[]).await;
    assert!(matches!(
        account_service::manage_list(&f.state, &snapshot, None, 10).await,
        Err(AccountError::Forbidden)
    ));
    assign(&f, &id, &[SystemPermission::AccountsRead]).await;
    f.request("POST", "/api/v1/auth/logout", None, Some(&cookie), false)
        .await;
    assert!(matches!(
        authorization::require(
            f.state.db_handles.writer(),
            &snapshot,
            SystemPermission::AccountsRead
        )
        .await,
        Err(AccountError::Unauthenticated)
    ));
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
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn read_only_holder_is_removable_but_last_critical_permission_holder_is_protected() {
    let f = Fixture::new().await;
    let (reader_id, cookie) = f.activate("reader").await;
    assign(
        &f,
        &reader_id,
        &[
            SystemPermission::AccountsRead,
            SystemPermission::SettingsRead,
        ],
    )
    .await;
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
        StatusCode::OK
    );
    let (delegate_id, _) = f.activate("delegate").await;
    assign(&f, &delegate_id, &[SystemPermission::AccountsCreate]).await;
    let operator_permissions: Vec<_> = SystemPermission::ALL
        .into_iter()
        .filter(|p| *p != SystemPermission::AccountsCreate)
        .collect();
    assign(&f, &f.initial_account_id, &operator_permissions).await;
    assert_eq!(
        f.request(
            "POST",
            &format!("/api/v1/management/accounts/{delegate_id}/lifecycle"),
            Some(json!({"action":"suspend"})),
            None,
            true
        )
        .await
        .1["code"],
        "account.last_operator"
    );
    let settings_only_id = f.initial_account_id.clone();
    assign(&f, &delegate_id, &SystemPermission::ALL).await;
    assign(&f, &settings_only_id, &[SystemPermission::SettingsUpdate]).await;
    // Another active holder now has both critical permissions, so the single-purpose operator can be retired.
    let delegate_cookie = f.login("delegate", PASSWORD).await;
    assert_eq!(
        f.request(
            "POST",
            &format!("/api/v1/management/accounts/{settings_only_id}/lifecycle"),
            Some(json!({"action":"suspend","current_password":PASSWORD})),
            Some(&delegate_cookie),
            false
        )
        .await
        .0,
        StatusCode::OK
    );
}

#[tokio::test]
async fn concurrent_operators_cannot_suspend_each_other_and_remove_all_critical_holders() {
    let f = Fixture::new().await;
    let (delegate_id, delegate_cookie) = f.activate("delegate").await;
    assign(&f, &delegate_id, &SystemPermission::ALL).await;
    let first_cookie = f.management_cookie.lock().unwrap().clone();
    let path_to_delegate = format!("/api/v1/management/accounts/{delegate_id}/lifecycle");
    let path_to_first = format!(
        "/api/v1/management/accounts/{}/lifecycle",
        f.initial_account_id
    );
    let body = json!({"action":"suspend","current_password":PASSWORD});
    let (a, b) = tokio::join!(
        f.request(
            "POST",
            &path_to_delegate,
            Some(body.clone()),
            Some(&first_cookie),
            false
        ),
        f.request(
            "POST",
            &path_to_first,
            Some(body),
            Some(&delegate_cookie),
            false
        )
    );
    assert_eq!(
        [a.0, b.0]
            .iter()
            .filter(|status| **status == StatusCode::OK)
            .count(),
        1
    );
    assert!([a.0, b.0].iter().all(|status| {
        [
            StatusCode::OK,
            StatusCode::UNAUTHORIZED,
            StatusCode::CONFLICT,
        ]
        .contains(status)
    }));
    for critical in [
        SystemPermission::AccountsCreate,
        SystemPermission::SettingsUpdate,
    ] {
        assert_eq!(
            system_permission_grant_repo::active_holders(f.state.db_handles.writer(), critical)
                .await
                .unwrap(),
            1
        );
    }
}

#[tokio::test]
async fn unknown_stored_permission_fails_closed_and_missing_accounts_cannot_receive_grants() {
    let f = Fixture::new().await;
    let (id, cookie) = f.activate("delegate").await;
    let db = f.state.db_handles.writer();
    let missing = AccountId::parse("00000000-0000-0000-0000-000000000000").unwrap();
    assert!(
        system_permission_grant_repo::grant(db, &missing, SystemPermission::AccountsRead, 0)
            .await
            .is_err()
    );
    assign(&f, &id, &[SystemPermission::AccountsRead]).await;
    grant::Entity::update_many()
        .col_expr(
            grant::Column::PermissionKey,
            Expr::value("astergate.system.accounts.*"),
        )
        .filter(grant::Column::AccountId.eq(&id))
        .exec(db)
        .await
        .unwrap();
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
        StatusCode::SERVICE_UNAVAILABLE
    );
    assert_eq!(
        f.request(
            "POST",
            &format!(
                "/api/v1/management/accounts/{}/lifecycle",
                f.initial_account_id
            ),
            Some(json!({"action":"delete","current_password":PASSWORD})),
            Some(&cookie),
            false
        )
        .await
        .0,
        StatusCode::SERVICE_UNAVAILABLE
    );
    assert_eq!(
        account::Entity::find_by_id(&f.initial_account_id)
            .one(db)
            .await
            .unwrap()
            .unwrap()
            .status,
        aster_gate::db::entities::types::StoredAccountStatus::Active
    );
}

#[tokio::test]
async fn grant_uniqueness_and_setup_do_not_accidentally_assign_privileges_to_registered_accounts() {
    let f = Fixture::new().await;
    let (id, _) = f.activate("ordinary").await;
    let db = f.state.db_handles.writer();
    let id = AccountId::parse(&id).unwrap();
    assert!(
        system_permission_grant_repo::list(db, &id)
            .await
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        system_permission_grant_repo::list(db, &AccountId::parse(&f.initial_account_id).unwrap())
            .await
            .unwrap()
            .len(),
        SystemPermission::ALL.len()
    );
    system_permission_grant_repo::grant(db, &id, SystemPermission::AccountsRead, 123)
        .await
        .unwrap();
    system_permission_grant_repo::grant(db, &id, SystemPermission::AccountsRead, 456)
        .await
        .unwrap();
    let rows = grant::Entity::find()
        .filter(grant::Column::AccountId.eq(id.as_str()))
        .all(db)
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].created_at, 123);
}
