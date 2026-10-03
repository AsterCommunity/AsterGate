use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "accounts")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub display_name: String,
    pub status: super::types::StoredAccountStatus,
    pub revision: i64,
    pub created_at: i64,
    pub updated_at: i64,
    pub suspended_from: Option<super::types::StoredAccountStatus>,
}
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}
impl ActiveModelBehavior for ActiveModel {}
