use crate::{
    db::entities::{account, system_permission_grant as grants, types::SystemPermissionValue},
    services::{account::models::AccountId, authorization::models::SystemPermission},
};
use sea_orm::{
    ColumnTrait, ConnectionTrait, DbErr, EntityTrait, JoinType, PaginatorTrait, QueryFilter,
    QuerySelect, Set,
};

pub async fn list<C: ConnectionTrait>(
    db: &C,
    id: &AccountId,
) -> Result<Vec<SystemPermission>, DbErr> {
    Ok(grants::Entity::find()
        .filter(grants::Column::AccountId.eq(id.as_str()))
        .all(db)
        .await?
        .into_iter()
        .map(|r| r.permission_key.into())
        .collect())
}

pub async fn grant<C: ConnectionTrait>(
    db: &C,
    id: &AccountId,
    permission: SystemPermission,
    now: i64,
) -> Result<(), DbErr> {
    grants::Entity::insert(grants::ActiveModel {
        account_id: Set(id.as_str().into()),
        permission_key: Set(permission.into()),
        created_at: Set(now),
    })
    .on_conflict_do_nothing()
    .exec(db)
    .await?;
    Ok(())
}

pub async fn remove<C: ConnectionTrait>(db: &C, id: &AccountId) -> Result<(), DbErr> {
    grants::Entity::delete_many()
        .filter(grants::Column::AccountId.eq(id.as_str()))
        .exec(db)
        .await?;
    Ok(())
}

pub async fn active_holders<C: ConnectionTrait>(
    db: &C,
    permission: SystemPermission,
) -> Result<u64, DbErr> {
    grants::Entity::find()
        .filter(grants::Column::PermissionKey.eq(SystemPermissionValue::from(permission)))
        .join(
            JoinType::InnerJoin,
            grants::Entity::belongs_to(account::Entity)
                .from(grants::Column::AccountId)
                .to(account::Column::Id)
                .into(),
        )
        .filter(account::Column::Status.eq(crate::db::entities::types::StoredAccountStatus::Active))
        .count(db)
        .await
}
