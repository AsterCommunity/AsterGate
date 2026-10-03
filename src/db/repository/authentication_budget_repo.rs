use crate::db::entities::authentication_budget as budget;
use sea_orm::{
    ColumnTrait, ConnectionTrait, DbErr, EntityTrait, ExprTrait, QueryFilter, Set,
    sea_query::{Expr, OnConflict},
};

pub async fn reserve<C: ConnectionTrait>(
    db: &C,
    key: String,
    expires_at: i64,
    limit: i32,
) -> Result<bool, DbErr> {
    budget::Entity::insert(budget::ActiveModel {
        key: Set(key.clone()),
        attempts: Set(0),
        expires_at: Set(expires_at),
    })
    .on_conflict(
        OnConflict::column(budget::Column::Key)
            .do_nothing_on([budget::Column::Key])
            .to_owned(),
    )
    .try_insert()
    .exec(db)
    .await?;
    Ok(budget::Entity::update_many()
        .col_expr(
            budget::Column::Attempts,
            Expr::col(budget::Column::Attempts).add(1),
        )
        .filter(budget::Column::Key.eq(key))
        .filter(budget::Column::Attempts.lt(limit))
        .exec(db)
        .await?
        .rows_affected
        == 1)
}
