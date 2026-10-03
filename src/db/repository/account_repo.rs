use crate::db::entities::{
    account, account_challenge as challenge, account_identifier as identifier,
    browser_session as session, password_credential as credential,
};
use crate::services::account::models::{AccountId, IdentifierKind, SessionId};
use sea_orm::PaginatorTrait;
use sea_orm::{
    ColumnTrait, ConnectionTrait, DbErr, EntityTrait, ExprTrait, QueryFilter, QueryOrder,
    QuerySelect, sea_query::Expr,
};

pub async fn count<C: ConnectionTrait>(db: &C) -> Result<u64, DbErr> {
    account::Entity::find().count(db).await
}

pub async fn find<C: ConnectionTrait>(
    db: &C,
    id: &AccountId,
) -> Result<Option<account::Model>, DbErr> {
    account::Entity::find_by_id(id.as_str()).one(db).await
}

pub async fn by_identifier<C: ConnectionTrait>(
    db: &C,
    kind: IdentifierKind,
    value: &str,
) -> Result<Option<account::Model>, DbErr> {
    let identifier = identifier::Entity::find()
        .filter(
            identifier::Column::Kind
                .eq(crate::db::entities::types::StoredIdentifierKind::from(kind)),
        )
        .filter(identifier::Column::Value.eq(value))
        .one(db)
        .await?;
    match identifier {
        Some(identifier) => {
            account::Entity::find_by_id(identifier.account_id)
                .one(db)
                .await
        }
        None => Ok(None),
    }
}

pub async fn identifiers<C: ConnectionTrait>(
    db: &C,
    id: &AccountId,
) -> Result<Vec<identifier::Model>, DbErr> {
    identifier::Entity::find()
        .filter(identifier::Column::AccountId.eq(id.as_str()))
        .all(db)
        .await
}

pub async fn password<C: ConnectionTrait>(
    db: &C,
    id: &AccountId,
) -> Result<Option<credential::Model>, DbErr> {
    credential::Entity::find_by_id(id.as_str()).one(db).await
}

/// The conditional revision update serializes all mutations of one Account, including login.
pub async fn claim_revision<C: ConnectionTrait>(
    db: &C,
    id: &AccountId,
    revision: i64,
    now: i64,
) -> Result<bool, DbErr> {
    Ok(account::Entity::update_many()
        .col_expr(
            account::Column::Revision,
            Expr::col(account::Column::Revision).add(1),
        )
        .col_expr(account::Column::UpdatedAt, Expr::value(now))
        .filter(account::Column::Id.eq(id.as_str()))
        .filter(account::Column::Revision.eq(revision))
        .exec(db)
        .await?
        .rows_affected
        == 1)
}

pub async fn session_by_token<C: ConnectionTrait>(
    db: &C,
    hash: &str,
    now: i64,
) -> Result<Option<session::Model>, DbErr> {
    session::Entity::find()
        .filter(session::Column::TokenHash.eq(hash))
        .filter(session::Column::RevokedAt.is_null())
        .filter(session::Column::ExpiresAt.gt(now))
        .one(db)
        .await
}

pub async fn sessions<C: ConnectionTrait>(
    db: &C,
    id: &AccountId,
    now: i64,
) -> Result<Vec<session::Model>, DbErr> {
    session::Entity::find()
        .filter(session::Column::AccountId.eq(id.as_str()))
        .filter(session::Column::RevokedAt.is_null())
        .filter(session::Column::ExpiresAt.gt(now))
        .order_by_desc(session::Column::CreatedAt)
        .limit(100)
        .all(db)
        .await
}

pub async fn revoke_session<C: ConnectionTrait>(
    db: &C,
    account: &AccountId,
    id: &SessionId,
    now: i64,
) -> Result<bool, DbErr> {
    Ok(session::Entity::update_many()
        .col_expr(session::Column::RevokedAt, Expr::value(now))
        .filter(session::Column::AccountId.eq(account.as_str()))
        .filter(session::Column::Id.eq(id.as_str()))
        .filter(session::Column::RevokedAt.is_null())
        .filter(session::Column::ExpiresAt.gt(now))
        .exec(db)
        .await?
        .rows_affected
        == 1)
}

pub async fn revoke_sessions<C: ConnectionTrait>(
    db: &C,
    id: &AccountId,
    now: i64,
) -> Result<(), DbErr> {
    session::Entity::update_many()
        .col_expr(session::Column::RevokedAt, Expr::value(now))
        .filter(session::Column::AccountId.eq(id.as_str()))
        .filter(session::Column::RevokedAt.is_null())
        .exec(db)
        .await?;
    Ok(())
}

pub async fn invalidate_challenges<C: ConnectionTrait>(
    db: &C,
    id: &AccountId,
    purpose: Option<crate::services::account::models::ChallengePurpose>,
    now: i64,
) -> Result<(), DbErr> {
    let mut query = challenge::Entity::update_many()
        .col_expr(challenge::Column::ConsumedAt, Expr::value(now))
        .filter(challenge::Column::AccountId.eq(id.as_str()))
        .filter(challenge::Column::ConsumedAt.is_null());
    if let Some(purpose) = purpose {
        query = query.filter(challenge::Column::Purpose.eq(
            crate::db::entities::types::StoredChallengePurpose::from(purpose),
        ));
    }
    query.exec(db).await?;
    Ok(())
}

pub async fn list<C: ConnectionTrait>(
    db: &C,
    after: Option<&AccountId>,
    limit: u64,
) -> Result<Vec<account::Model>, DbErr> {
    let mut query = account::Entity::find()
        .order_by_asc(account::Column::Id)
        .limit(limit);
    if let Some(after) = after {
        query = query.filter(account::Column::Id.gt(after.as_str()));
    }
    query.all(db).await
}
