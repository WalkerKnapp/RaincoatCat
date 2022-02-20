use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "optional_roles")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub role_id: i64,
    pub server_id: i64,
    pub emoji: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
