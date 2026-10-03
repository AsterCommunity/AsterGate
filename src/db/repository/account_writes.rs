use crate::db::entities::{
    account, account_challenge as challenge, account_identifier as identifier,
    browser_session as session, password_credential as credential,
};
use crate::services::account::models::{AccountId, AccountStatus, IdentifierKind};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DbErr, EntityTrait, QueryFilter, Set,
    sea_query::Expr,
};

pub struct NewAccountRecord {
    pub display_name: String,
    pub username: String,
    pub email: String,
    pub password_hash: String,
}

pub async fn create_pending<C: ConnectionTrait>(
    db: &C,
    id: &AccountId,
    input: NewAccountRecord,
    now: i64,
) -> Result<account::Model, DbErr> {
    let row = account::ActiveModel {
        id: Set(id.as_str().into()),
        display_name: Set(input.display_name),
        status: Set(AccountStatus::PendingVerification.into()),
        revision: Set(0),
        created_at: Set(now),
        updated_at: Set(now),
        suspended_from: Set(None),
    }
    .insert(db)
    .await?;
    for (kind, value) in [
        (IdentifierKind::Username, input.username),
        (IdentifierKind::Email, input.email),
    ] {
        identifier::ActiveModel {
            id: Set(uuid::Uuid::new_v4().to_string()),
            account_id: Set(id.as_str().into()),
            kind: Set(kind.into()),
            value: Set(value),
            verified_at: Set(None),
        }
        .insert(db)
        .await?;
    }
    credential::ActiveModel {
        account_id: Set(id.as_str().into()),
        password_hash: Set(input.password_hash),
        updated_at: Set(now),
    }
    .insert(db)
    .await?;
    Ok(row)
}

pub async fn create_session<C: ConnectionTrait>(
    db: &C,
    account: &AccountId,
    token_hash: String,
    user_agent: String,
    now: i64,
    expires_at: i64,
) -> Result<(), DbErr> {
    session::ActiveModel {
        id: Set(uuid::Uuid::new_v4().to_string()),
        account_id: Set(account.as_str().into()),
        token_hash: Set(token_hash),
        user_agent: Set(user_agent),
        created_at: Set(now),
        expires_at: Set(expires_at),
        revoked_at: Set(None),
    }
    .insert(db)
    .await?;
    Ok(())
}

pub async fn update_profile<C: ConnectionTrait>(
    db: &C,
    id: &AccountId,
    display_name: String,
) -> Result<(), DbErr> {
    account::Entity::update_many()
        .col_expr(account::Column::DisplayName, Expr::value(display_name))
        .filter(account::Column::Id.eq(id.as_str()))
        .exec(db)
        .await?;
    Ok(())
}

pub async fn update_status<C: ConnectionTrait>(
    db: &C,
    id: &AccountId,
    status: AccountStatus,
    suspended_from: Option<AccountStatus>,
) -> Result<(), DbErr> {
    account::Entity::update_many()
        .col_expr(
            account::Column::Status,
            Expr::value(crate::db::entities::types::StoredAccountStatus::from(
                status,
            )),
        )
        .col_expr(
            account::Column::SuspendedFrom,
            Expr::value(suspended_from.map(crate::db::entities::types::StoredAccountStatus::from)),
        )
        .filter(account::Column::Id.eq(id.as_str()))
        .exec(db)
        .await?;
    Ok(())
}

pub async fn update_password<C: ConnectionTrait>(
    db: &C,
    id: &AccountId,
    password_hash: String,
    now: i64,
) -> Result<(), DbErr> {
    credential::Entity::update_many()
        .col_expr(credential::Column::PasswordHash, Expr::value(password_hash))
        .col_expr(credential::Column::UpdatedAt, Expr::value(now))
        .filter(credential::Column::AccountId.eq(id.as_str()))
        .exec(db)
        .await?;
    Ok(())
}

pub async fn erase_records<C: ConnectionTrait>(db: &C, id: &AccountId) -> Result<(), DbErr> {
    super::system_permission_grant_repo::remove(db, id).await?;
    identifier::Entity::delete_many()
        .filter(identifier::Column::AccountId.eq(id.as_str()))
        .exec(db)
        .await?;
    credential::Entity::delete_by_id(id.as_str())
        .exec(db)
        .await?;
    session::Entity::delete_many()
        .filter(session::Column::AccountId.eq(id.as_str()))
        .exec(db)
        .await?;
    challenge::Entity::delete_many()
        .filter(challenge::Column::AccountId.eq(id.as_str()))
        .exec(db)
        .await?;
    update_profile(db, id, String::new()).await?;
    update_status(db, id, AccountStatus::Deleted, None).await
}

pub async fn verify_email<C: ConnectionTrait>(
    db: &C,
    id: &AccountId,
    expected: &str,
    now: i64,
) -> Result<bool, DbErr> {
    Ok(identifier::Entity::update_many()
        .col_expr(identifier::Column::VerifiedAt, Expr::value(now))
        .filter(identifier::Column::AccountId.eq(id.as_str()))
        .filter(
            identifier::Column::Kind.eq(crate::db::entities::types::StoredIdentifierKind::Email),
        )
        .filter(identifier::Column::Value.eq(expected))
        .filter(identifier::Column::VerifiedAt.is_null())
        .exec(db)
        .await?
        .rows_affected
        == 1)
}

pub async fn replace_email<C: ConnectionTrait>(
    db: &C,
    id: &AccountId,
    value: &str,
    now: i64,
) -> Result<(), DbErr> {
    identifier::Entity::update_many()
        .col_expr(identifier::Column::Value, Expr::value(value))
        .col_expr(identifier::Column::VerifiedAt, Expr::value(now))
        .filter(identifier::Column::AccountId.eq(id.as_str()))
        .filter(identifier::Column::Kind.eq("email"))
        .exec(db)
        .await?;
    Ok(())
}
