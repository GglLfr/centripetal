use avian2d::prelude::*;
use bevy::{
    ecs::{query::QueryItem, system::SystemParamItem},
    prelude::*,
};

use crate::logic::{CameraTarget, FromLevelEntity, LevelEntity, PenumbraEntity, entities::penumbra::AttractorInitial};

#[derive(Debug, Copy, Clone, Default, Component)]
#[require(CameraTarget, PenumbraEntity)]
pub struct SelenePenumbra;
impl FromLevelEntity for SelenePenumbra {
    type Param = ();
    type Data = ();

    fn from_level_entity(
        mut e: EntityCommands,
        entity: &LevelEntity,
        _: &mut SystemParamItem<Self::Param>,
        _: QueryItem<Self::Data>,
    ) -> Result {
        let ccw = entity.bool("ccw")?;

        e.insert((Self, AttractorInitial { ccw }, Collider::circle(8.)));
        Ok(())
    }
}
