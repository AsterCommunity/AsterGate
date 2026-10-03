use super::types::SystemPermissionValue;
use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "system_permission_grants")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub account_id: String,
    #[sea_orm(primary_key, auto_increment = false)]
    pub permission_key: SystemPermissionValue,
    pub created_at: i64,
}
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}
impl ActiveModelBehavior for ActiveModel {}
