use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "servers")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub mod_role_id: i64,

    pub verified_role_id: Option<i64>,
    pub verification_message_id: Option<i64>,
    pub verification_emoji: Option<String>,
    pub verification_timeout: Option<i64>, // in hours

    pub dunce_role_id: Option<i64>
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
