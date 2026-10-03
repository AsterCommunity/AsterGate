use sea_orm::entity::prelude::*;
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "account_challenges")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub account_id: String,
    pub purpose: super::types::StoredChallengePurpose,
    pub token_hash: String,
    pub target_email: String,
    pub created_at: i64,
    pub expires_at: i64,
    pub consumed_at: Option<i64>,
    pub outbox_id: Option<i64>,
}
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}
impl ActiveModelBehavior for ActiveModel {}
