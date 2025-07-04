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
    logic::{
        Fields, FromLevelEntity,
        entities::{
            Hurt,
            penumbra::{AttractedInitial, OnLaunch, PenumbraEntity},
        },
    },
};

#[derive(Debug, Copy, Clone, Default, Component)]
#[require(PenumbraEntity)]
pub struct ThornPillar;
impl FromLevelEntity for ThornPillar {
    type Param = ();
    type Data = Write<Transform>;

    fn from_level_entity(
        mut e: EntityCommands,
        fields: &Fields,
        _: &mut SystemParamItem<Self::Param>,
        mut trns: QueryItem<Self::Data>,
    ) -> Result {
        let length = fields.int("length")?;
        let ccw = fields.bool("ccw")?;
        let facing = fields.point_px("facing")?.as_vec2();

        trns.rotation = Quat::from_axis_angle(Vec3::Z, (facing - trns.translation.truncate()).to_angle());
        e.insert((
            Self,
            AttractedInitial { ccw },
            Collider::rectangle(length as f32 * PIXELS_PER_UNIT as f32, PIXELS_PER_UNIT as f32 / 2.),
        ))
        .observe(OnLaunch::collide(true, |mut e, by| {
            e.trigger(Hurt::new(by, 1));
        }));

        debug!("Spawned thorn pillar {}!", e.id());
        Ok(())
    }
}
