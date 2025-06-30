use avian2d::prelude::*;
use bevy::{
    ecs::{query::QueryItem, system::SystemParamItem},
    prelude::*,
};

use crate::{
    PIXELS_PER_UNIT,
    logic::{FromLevelEntity, LevelEntity, PenumbraEntity, entities::penumbra::AttractorInitial},
};

#[derive(Debug, Copy, Clone, Default, Component)]
#[require(PenumbraEntity)]
pub struct ThornPillar;
impl FromLevelEntity for ThornPillar {
    type Param = ();
    type Data = ();

    fn from_level_entity(
        mut e: EntityCommands,
        entity: &LevelEntity,
        _: &mut SystemParamItem<Self::Param>,
        _: QueryItem<Self::Data>,
    ) -> Result {
        let length = entity.int("length")?;
        let ccw = entity.bool("ccw")?;

        e.insert((
            Self,
            AttractorInitial { ccw },
            Collider::rectangle(PIXELS_PER_UNIT, length as f32 * PIXELS_PER_UNIT),
        ));

        debug!("Spawned thorn pillar {}!", e.id());
        Ok(())
    }
}
