//! Migration smoke tests.

use aster_forge_test::temp::SqliteTestDatabase;
use sea_orm::Database;
use sea_orm_migration::prelude::MigratorTrait;
use sea_orm_migration::prelude::SchemaManager;

#[test]
fn foundation_migration_is_registered_first() {
    let migrations = aster_gate_migration::Migrator::migrations();

    assert_eq!(migrations.len(), 2);
    assert_eq!(
        migrations[0].name(),
        "m20260627_000001_forge_foundation_schema"
    );
}

#[tokio::test]
async fn foundation_migration_applies_and_rolls_back_on_sqlite() {
    let database = SqliteTestDatabase::new("foundation-migration");
    let db = Database::connect(database.url())
        .await
        .expect("connect sqlite migration test database");

    aster_gate_migration::Migrator::up(&db, None)
        .await
        .expect("apply foundation migration");
    let manager = SchemaManager::new(&db);
    assert!(!manager.has_table("account_capabilities").await.unwrap());

    for table in [
        "runtime_leases",
        "scheduled_tasks",
        "system_config",
        "mail_outbox",
        "audit_logs",
        "accounts",
        "system_permission_grants",
        "account_identifiers",
        "password_credentials",
        "browser_sessions",
        "account_challenges",
        "authentication_budgets",
    ] {
        assert!(
            manager
                .has_table(table)
                .await
                .expect("query migrated table"),
            "expected {table} to be created"
        );
    }

    aster_gate_migration::Migrator::down(&db, None)
        .await
        .expect("roll back foundation migration");
    for table in [
        "runtime_leases",
        "scheduled_tasks",
        "system_config",
        "mail_outbox",
        "audit_logs",
        "accounts",
        "system_permission_grants",
        "account_identifiers",
        "password_credentials",
        "browser_sessions",
        "account_challenges",
        "authentication_budgets",
    ] {
        assert!(
            !manager
                .has_table(table)
                .await
                .expect("query rolled back table"),
            "expected {table} to be dropped"
        );
    }
    db.close()
        .await
        .expect("close sqlite migration test database");
}

#[tokio::test]
async fn permission_grants_schema_enforces_owner_unique_key_and_is_restart_idempotent() {
    use aster_gate::db::entities::{
        account, system_permission_grant as grant,
        types::{StoredAccountStatus, SystemPermissionValue},
    };
    use sea_orm::{
        ActiveModelTrait, ConnectionTrait, EntityTrait, PaginatorTrait, Set,
        sea_query::{Alias, Query},
    };
    let database = SqliteTestDatabase::new("permission-schema");
    let db = Database::connect(database.url()).await.unwrap();
    db.execute_unprepared("PRAGMA foreign_keys = ON")
        .await
        .unwrap();
    aster_gate_migration::Migrator::up(&db, None).await.unwrap();
    let id = uuid::Uuid::new_v4().to_string();
    account::ActiveModel {
        id: Set(id.clone()),
        display_name: Set("Operator".into()),
        status: Set(StoredAccountStatus::Active),
        revision: Set(0),
        created_at: Set(123),
        updated_at: Set(123),
        suspended_from: Set(None),
    }
    .insert(&db)
    .await
    .unwrap();
    let row = grant::ActiveModel {
        account_id: Set(id.clone()),
        permission_key: Set(SystemPermissionValue::AccountsSuspend),
        created_at: Set(123),
    };
    row.clone().insert(&db).await.unwrap();
    assert!(row.clone().insert(&db).await.is_err());
    assert!(
        grant::ActiveModel {
            account_id: Set(uuid::Uuid::new_v4().to_string()),
            ..row
        }
        .insert(&db)
        .await
        .is_err()
    );
    let id_column = Alias::new("account_id");
    let permission_column = Alias::new("permission_key");
    let query = Query::insert()
        .into_table(Alias::new("system_permission_grants"))
        .columns([id_column, permission_column, Alias::new("created_at")])
        .values([
            id.into(),
            "astergate.system.accounts.suspend".into(),
            456.into(),
        ])
        .unwrap()
        .to_owned();
    assert!(
        db.execute_raw(db.get_database_backend().build(&query))
            .await
            .is_err()
    );
    aster_gate_migration::Migrator::up(&db, None).await.unwrap();
    assert_eq!(grant::Entity::find().count(&db).await.unwrap(), 1);
    assert_eq!(
        grant::Entity::find()
            .one(&db)
            .await
            .unwrap()
            .unwrap()
            .created_at,
        123
    );
}
