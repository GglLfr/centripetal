use std::f32::consts::TAU;

use crate::{
    Sprites,
    graphics::{Animation, AnimationHooks, AnimationSmoothing, BaseColor},
    logic::{
        Fields, FromLevelEntity, Level, LevelUnload,
        entities::{
            EntityLayers, TryHurt,
            penumbra::{AttractedInitial, PenumbraEntity, TryLaunch},
        },
    },
    math::{Interp, RngExt as _},
    prelude::*,
};

#[derive(Debug, Copy, Clone, Default, Component)]
#[require(PenumbraEntity, ThornRingTimers, CollisionLayers = EntityLayers::penumbra_hostile(), CollisionEventsEnabled, DebugRender::none())]
pub struct ThornRing {
    pub radius: f32,
    pub opening: f32,
}

#[derive(Debug, Copy, Clone, Default, Component)]
pub struct ThornRingTimers {
    pub particle: f32,
}

#[derive(Debug, Copy, Clone, Default, Component)]
pub struct ThornRingParticle;

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

pub fn update_thorn_ring_timers(
    mut commands: Commands,
    time: Res<Time>,
    sprites: Res<Sprites>,
    mut rings: Query<(&mut ThornRingTimers, &ThornRing, &GlobalTransform)>,
    mut rng: Local<Rng>,
    level: Query<Entity, (With<Level>, Without<LevelUnload>)>,
) {
    let Ok(level_entity) = level.single() else { return };
    let dt = time.delta_secs();

    for (mut timers, &ring, &trns) in &mut rings {
        timers.particle += dt;

        let particle_period = 1. / (2. * (TAU - ring.opening) * ring.radius / 24.);
        let particles = timers.particle / particle_period;
        if particles >= 1. {
            let count = particles as usize;
            timers.particle = particles.fract() * particle_period;

            for (angle, offset) in rng
                .fork()
                .len_vectors(count, ring.opening / 2., TAU - ring.opening / 2., ring.radius + 1., ring.radius + 1.)
            {
                let local_trns = trns.compute_transform()
                    * Transform {
                        translation: vec3(offset.x + ring.radius, offset.y, 0.),
                        rotation: Quat::from_axis_angle(Vec3::Z, angle.as_radians()),
                        scale: vec3(1., if rng.bool() { 1. } else { -1. }, 1.),
                    };

                commands.spawn((
                    ChildOf(level_entity),
                    ThornRingParticle,
                    local_trns,
                    GlobalTransform::from(local_trns),
                    BaseColor(Color::linear_rgb(1., 4., 2.)),
                    Animation::new(sprites.thorn.clone_weak(), "anim"),
                    AnimationSmoothing(Interp::Identity),
                    AnimationHooks::despawn_on_done("anim"),
                ));
            }
        }
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
