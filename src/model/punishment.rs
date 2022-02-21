use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "String", db_type = "Enum", enum_name = "punishment_type")]
pub enum PunishmentType {
    #[sea_orm(string_value = "dunce")]
    Dunce,
    #[sea_orm(string_value = "ban")]
    Ban
}

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "punishments")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub user_id: i64,
    pub server_id: i64,
    pub punishment_type: PunishmentType,
    pub expires: Option<DateTime>
}

#[derive(Copy, Clone, Debug, EnumIter)]
pub enum Relation {
    PunishmentRemovedRole
}

impl RelationTrait for Relation {
    fn def(&self) -> RelationDef {
        match self {
            Self::PunishmentRemovedRole => Entity::has_many(super::punishment_removed_role::Entity).into(),
        }
    }
}

impl Related<super::punishment_removed_role::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::PunishmentRemovedRole.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
