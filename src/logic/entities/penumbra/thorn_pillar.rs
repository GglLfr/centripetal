use avian2d::prelude::*;
use bevy::{
    ecs::{
        query::QueryItem,
        system::{SystemParamItem, lifetimeless::Write},
    },
    prelude::*,
};

use crate::{
    PIXELS_PER_UNIT,
    logic::{FromLevelEntity, LevelEntity, PenumbraEntity, entities::penumbra::AttractedInitial},
};

#[derive(Debug, Copy, Clone, Default, Component)]
#[require(PenumbraEntity)]
pub struct ThornPillar;
impl FromLevelEntity for ThornPillar {
    type Param = ();
    type Data = Write<Transform>;

    fn from_level_entity(
        mut e: EntityCommands,
        entity: &LevelEntity,
        _: &mut SystemParamItem<Self::Param>,
        mut trns: QueryItem<Self::Data>,
    ) -> Result {
        let length = entity.int("length")?;
        let ccw = entity.bool("ccw")?;
        let facing = entity.point_px("facing")?.as_vec2();

        trns.rotation = Quat::from_axis_angle(Vec3::Z, (facing - trns.translation.truncate()).to_angle());
        e.insert((
            Self,
            AttractedInitial { ccw },
            Collider::rectangle(length as f32 * PIXELS_PER_UNIT as f32, PIXELS_PER_UNIT as f32 / 2.),
        ));

        debug!("Spawned thorn pillar {}!", e.id());
        Ok(())
    }
}
