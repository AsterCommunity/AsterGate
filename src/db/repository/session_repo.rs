//! Writer-side session records and fenced rotation. Product decisions stay in services.
use crate::db::entities::{account, browser_session as session};
use crate::services::account::models::{AccountId, SessionId};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DbErr, EntityTrait, ExprTrait, QueryFilter,
    QuerySelect, sea_query::Expr,
};

/// A recorded initial migration is not rerun when an unreleased development schema changes.
/// Validate column availability without loading identities or silently resetting existing data.
pub async fn validate_schema<C: ConnectionTrait>(db: &C) -> Result<(), DbErr> {
    account::Entity::find().limit(0).all(db).await?;
    session::Entity::find().limit(0).all(db).await?;
    Ok(())
}

/// Serializes refresh with every account security mutation without advancing its business revision.
pub async fn lock_account<C: ConnectionTrait>(
    db: &C,
    id: &AccountId,
) -> Result<Option<account::Model>, DbErr> {
    account::Entity::update_many()
        .col_expr(
            account::Column::SessionVersion,
            Expr::col(account::Column::SessionVersion),
        )
        .filter(account::Column::Id.eq(id.as_str()))
        .exec(db)
        .await?;
    super::account_repo::find(db, id).await
}

pub async fn find<C: ConnectionTrait>(
    db: &C,
    account: &AccountId,
    id: &SessionId,
) -> Result<Option<session::Model>, DbErr> {
    session::Entity::find()
        .filter(session::Column::Id.eq(id.as_str()))
        .filter(session::Column::AccountId.eq(account.as_str()))
        .one(db)
        .await
}

pub async fn create<C: ConnectionTrait>(db: &C, row: session::Model) -> Result<(), DbErr> {
    use sea_orm::IntoActiveModel;
    row.into_active_model().insert(db).await?;
    Ok(())
}

/// The immutable expires_at is deliberately never updated during rotation.
pub async fn rotate<C: ConnectionTrait>(
    db: &C,
    request: &aster_forge_auth::RefreshRotationRequest,
) -> Result<bool, DbErr> {
    Ok(session::Entity::update_many()
        .col_expr(
            session::Column::CurrentRefreshJti,
            Expr::value(request.next_refresh_jti.as_str()),
        )
        .col_expr(
            session::Column::PreviousRefreshJti,
            Expr::value(Some(request.current_refresh_jti.as_str())),
        )
        .col_expr(session::Column::LastRotatedAt, Expr::value(request.now))
        .filter(session::Column::Id.eq(request.session_id.as_str()))
        .filter(session::Column::AccountId.eq(request.subject.as_str()))
        .filter(
            session::Column::SessionVersion.eq(i64::try_from(request.session_version.value())
                .map_err(|_| DbErr::Custom("invalid session version".into()))?),
        )
        .filter(session::Column::CurrentRefreshJti.eq(request.current_refresh_jti.as_str()))
        .filter(session::Column::RevokedAt.is_null())
        .filter(session::Column::ExpiresAt.gt(request.now))
        .exec(db)
        .await?
        .rows_affected
        == 1)
}

pub async fn bump_version<C: ConnectionTrait>(db: &C, id: &AccountId) -> Result<(), DbErr> {
    let result = account::Entity::update_many()
        .col_expr(
            account::Column::SessionVersion,
            Expr::col(account::Column::SessionVersion).add(1),
        )
        .filter(account::Column::Id.eq(id.as_str()))
        .filter(account::Column::SessionVersion.gte(0))
        .filter(account::Column::SessionVersion.lt(i64::MAX))
        .exec(db)
        .await?;
    if result.rows_affected != 1 {
        return Err(DbErr::Custom(
            "account security version exhausted or unavailable".into(),
        ));
    }
    Ok(())
}
