use sea_orm::entity::prelude::*;
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "password_credentials")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub account_id: String,
    pub password_hash: String,
    pub updated_at: i64,
}
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}
impl ActiveModelBehavior for ActiveModel {}
