use std::f32::consts::TAU;

use crate::{
    logic::{
        Fields, FromLevelEntity,
        entities::{
            EntityLayers, TryHurt,
            penumbra::{AttractedInitial, PenumbraEntity, TryLaunch},
        },
    },
    prelude::*,
};

#[derive(Debug, Copy, Clone, Default, Component)]
#[require(PenumbraEntity, CollisionLayers = EntityLayers::penumbra_hostile(), CollisionEventsEnabled, DebugRender::none())]
pub struct ThornRing {
    pub radius: f32,
    pub opening: f32,
}

impl FromLevelEntity for ThornRing {
    type Param = ();
    type Data = Write<Transform>;

    fn from_level_entity(mut e: EntityCommands, fields: &Fields, _: &mut SystemParamItem<Self::Param>, mut trns: QueryItem<Self::Data>) -> Result {
        let ccw = fields.bool("ccw")?;
        let facing = fields.point_px("facing")?.as_vec2();
        let opening = fields.float("opening")?.to_radians();

        let pos = trns.translation.xy();
        let radius = (facing - pos).length();
        let resolution = radius as usize;

        let mut vertices = Vec::new();
        for i in 0..=resolution {
            let angle = (opening / 2.).lerp(TAU - opening / 2., i as f32 / resolution as f32);
            let (sin, cos) = angle.sin_cos();
            vertices.push(Vec2::new(cos * radius + radius, sin * radius));
        }

        trns.rotation = Quat::from_axis_angle(Vec3::Z, (facing - trns.translation.xy()).to_angle());
        e.insert((Self { radius, opening }, AttractedInitial { ccw }, Collider::polyline(vertices, None)))
            .observe(|trigger: Trigger<OnCollisionStart>, mut commands: Commands| {
                if let Some(body) = trigger.body {
                    commands
                        .entity(body)
                        .queue_handled(TryHurt::by(trigger.target(), i32::MAX as u32), ignore);
                }
            })
            .observe(|mut trigger: Trigger<TryLaunch>, mut commands: Commands| {
                if trigger.by() != trigger.target() {
                    trigger.event_mut().stop();
                    commands.entity(trigger.by()).queue_handled(TryHurt::by(trigger.target(), 1), ignore);
                }
            });

        Ok(())
    }
}

pub fn draw_thorn_ring(mut shapes: ShapePainter, rings: Query<(&GlobalTransform, &ThornRing)>) {
    for (&trns, &ring) in &rings {
        shapes.transform = (trns * Transform::from_xyz(ring.radius, 0., 0.)).compute_transform();
        shapes.color = Color::linear_rgb(1., 4., 2.);
        shapes.thickness = 1.;
        shapes.hollow = true;
        shapes.arc(ring.radius, ring.opening / 2., TAU - ring.opening / 2.);
    }
}
