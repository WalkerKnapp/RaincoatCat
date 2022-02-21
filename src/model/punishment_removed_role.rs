use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "punishment_removed_roles")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub punishment_id: i64,
    pub role_id: i64
}

#[derive(Copy, Clone, Debug, EnumIter)]
pub enum Relation {
    Punishment
}

impl RelationTrait for Relation {
    fn def(&self) -> RelationDef {
        match self {
            Self::Punishment => Entity::belongs_to(super::punishment::Entity)
                .from(Column::PunishmentId)
                .to(super::punishment::Column::Id)
                .into(),
        }
    }
}

impl Related<super::punishment::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Punishment.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
