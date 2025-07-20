use avian2d::prelude::*;
use bevy::{
    ecs::{
        query::QueryItem,
        system::{SystemParamItem, lifetimeless::Write},
    },
    prelude::*,
};

use crate::logic::{
    Fields, FromLevelEntity,
    entities::penumbra::{AttractedInitial, PenumbraEntity},
};

#[derive(Debug, Copy, Clone, Default, Component)]
#[require(PenumbraEntity)]
pub struct GenericPenumbra;
impl FromLevelEntity for GenericPenumbra {
    type Param = ();
    type Data = Write<Transform>;

    fn from_level_entity(
        mut e: EntityCommands,
        fields: &Fields,
        _: &mut SystemParamItem<Self::Param>,
        mut trns: QueryItem<Self::Data>,
    ) -> Result {
        let facing = fields.point_px("facing")?.as_vec2();
        let ccw = fields.bool("ccw")?;

        trns.rotation =
            Quat::from_axis_angle(Vec3::Z, (facing - trns.translation.truncate()).to_angle());
        e.insert((Self, AttractedInitial { ccw }, Collider::circle(5.)));

        debug!("Spawned generic penumbra entity {}!", e.id());
        Ok(())
    }
}
