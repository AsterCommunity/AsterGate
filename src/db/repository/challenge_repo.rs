use crate::{
    db::entities::{
        account_challenge as challenge, authentication_budget as budget, browser_session as session,
    },
    services::account::models::{AccountId, ChallengePurpose},
};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DbErr, EntityTrait, QueryFilter, QuerySelect,
    Set, sea_query::Expr,
};

pub struct NewChallengeRecord {
    pub id: String,
    pub account_id: AccountId,
    pub purpose: ChallengePurpose,
    pub token_hash: String,
    pub target_email: String,
    pub created_at: i64,
    pub expires_at: i64,
    pub outbox_id: i64,
}

pub async fn create<C: ConnectionTrait>(db: &C, input: NewChallengeRecord) -> Result<(), DbErr> {
    challenge::ActiveModel {
        id: Set(input.id),
        account_id: Set(input.account_id.as_str().into()),
        purpose: Set(input.purpose.into()),
        token_hash: Set(input.token_hash),
        target_email: Set(input.target_email),
        created_at: Set(input.created_at),
        expires_at: Set(input.expires_at),
        consumed_at: Set(None),
        outbox_id: Set(Some(input.outbox_id)),
    }
    .insert(db)
    .await?;
    Ok(())
}

pub async fn for_account<C: ConnectionTrait>(
    db: &C,
    id: &AccountId,
    purpose: Option<ChallengePurpose>,
) -> Result<Vec<challenge::Model>, DbErr> {
    let mut query = challenge::Entity::find().filter(challenge::Column::AccountId.eq(id.as_str()));
    if let Some(purpose) = purpose {
        query = query.filter(challenge::Column::Purpose.eq(
            crate::db::entities::types::StoredChallengePurpose::from(purpose),
        ));
    }
    query.all(db).await
}

pub async fn live_by_hash<C: ConnectionTrait>(
    db: &C,
    hash: &str,
    purpose: ChallengePurpose,
    now: i64,
) -> Result<Option<challenge::Model>, DbErr> {
    challenge::Entity::find()
        .filter(challenge::Column::TokenHash.eq(hash))
        .filter(challenge::Column::Purpose.eq(purpose.as_str()))
        .filter(challenge::Column::ConsumedAt.is_null())
        .filter(challenge::Column::ExpiresAt.gt(now))
        .one(db)
        .await
}

pub async fn consume<C: ConnectionTrait>(db: &C, id: &str, now: i64) -> Result<bool, DbErr> {
    Ok(challenge::Entity::update_many()
        .col_expr(challenge::Column::ConsumedAt, Expr::value(now))
        .filter(challenge::Column::Id.eq(id))
        .filter(challenge::Column::ConsumedAt.is_null())
        .filter(challenge::Column::ExpiresAt.gt(now))
        .exec(db)
        .await?
        .rows_affected
        == 1)
}

pub async fn redact_and_cancel_mail<C: ConnectionTrait>(db: &C, id: i64) -> Result<(), DbErr> {
    use aster_forge_db::mail_outbox as outbox;
    use aster_forge_mail::MailOutboxStatus;
    outbox::Entity::update_many()
        .col_expr(
            outbox::Column::Status,
            Expr::value(MailOutboxStatus::Failed),
        )
        .filter(outbox::Column::Id.eq(id))
        .filter(outbox::Column::Status.is_in([
            MailOutboxStatus::Pending,
            MailOutboxStatus::Processing,
            MailOutboxStatus::Retry,
        ]))
        .exec(db)
        .await?;
    outbox::Entity::update_many()
        .col_expr(
            outbox::Column::PayloadJson,
            Expr::value(aster_forge_mail::StoredMailPayload::cleared()),
        )
        .col_expr(outbox::Column::ToAddress, Expr::value(""))
        .filter(outbox::Column::Id.eq(id))
        .exec(db)
        .await?;
    Ok(())
}

pub async fn cleanup<C: ConnectionTrait>(db: &C, now: i64) -> Result<(), DbErr> {
    budget::Entity::delete_many()
        .filter(budget::Column::ExpiresAt.lte(now))
        .exec(db)
        .await?;
    session::Entity::delete_many()
        .filter(session::Column::ExpiresAt.lte(now))
        .exec(db)
        .await?;
    let expired = challenge::Entity::find()
        .filter(challenge::Column::ExpiresAt.lt(now - 86400))
        .limit(500)
        .all(db)
        .await?;
    for c in expired {
        if let Some(id) = c.outbox_id {
            aster_forge_db::mail_outbox::Entity::delete_by_id(id)
                .exec(db)
                .await?;
        }
        challenge::Entity::delete_by_id(c.id).exec(db).await?;
    }
    Ok(())
}
