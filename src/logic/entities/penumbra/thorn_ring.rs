use avian2d::{math::PI, prelude::*};
use bevy::{
    ecs::{
        query::QueryItem,
        system::{SystemParamItem, lifetimeless::Write},
    },
    prelude::*,
};

use crate::logic::{
    Fields, FromLevelEntity,
    entities::{
        TryHurt,
        penumbra::{AttractedInitial, PenumbraEntity, TryLaunch},
    },
};

#[derive(Debug, Copy, Clone, Default, Component)]
#[require(PenumbraEntity)]
pub struct ThornRing;
impl FromLevelEntity for ThornRing {
    type Param = ();
    type Data = Write<Transform>;

    fn from_level_entity(
        mut e: EntityCommands,
        fields: &Fields,
        _: &mut SystemParamItem<Self::Param>,
        mut trns: QueryItem<Self::Data>,
    ) -> Result {
        let ccw = fields.bool("ccw")?;
        let facing = fields.point_px("facing")?.as_vec2();
        let opening = fields.float("opening")?.to_radians();

        let pos = trns.translation.xy();
        let radius = (facing - pos).length();
        let resolution = radius as usize;

        let mut vertices = Vec::new();
        for i in 0..=resolution {
            let angle = (opening / 2.).lerp(PI * 2. - opening / 2., i as f32 / resolution as f32);
            let (sin, cos) = angle.sin_cos();
            vertices.push(Vec2::new(cos * radius + radius, sin * radius));
        }

        trns.rotation = Quat::from_axis_angle(Vec3::Z, (facing - trns.translation.xy()).to_angle());
        e.insert((
            Self,
            AttractedInitial { ccw },
            Collider::polyline(vertices, None),
        ))
        .observe(|mut trigger: Trigger<TryLaunch>, mut commands: Commands| {
            if trigger.by() != trigger.target() {
                trigger.event_mut().stop();
                commands
                    .entity(trigger.by())
                    .queue(TryHurt::by(trigger.target(), 1));
            }
        });

        Ok(())
    }
}
