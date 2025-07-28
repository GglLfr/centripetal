use std::{f32::consts::PI, time::Duration};

use avian2d::{dynamics::solver::solver_body::SolverBody, prelude::*};
use bevy::{
    ecs::{
        query::QueryItem,
        system::{SystemParamItem, lifetimeless::SRes},
    },
    prelude::*,
};
use leafwing_input_manager::prelude::*;
use serde::{Deserialize, Serialize};

use crate::{
    Sprites,
    graphics::{Animation, AnimationMode, EntityColor, SpriteDrawer, SpriteSection},
    logic::{
        Fields, FromLevelEntity,
        entities::{
            TryHurt,
            penumbra::{LaunchTarget, PenumbraEntity},
        },
    },
    math::DurationExt,
};

#[derive(Debug, Copy, Clone, Default, Component)]
pub struct NoAttract;

#[derive(Debug, Clone, Component)]
#[require(PenumbraEntity, AttractorEntities, SpriteDrawer)]
pub struct Attractor {
    pub radius: f32,
    pub gravity: f32,
    pub caster: Collider,
}

#[derive(Debug, Clone, Default, Component, Deref, DerefMut)]
pub struct AttractorEntities(pub Vec<Entity>);

impl FromLevelEntity for Attractor {
    type Param = SRes<Sprites>;
    type Data = ();

    fn from_level_entity(
        mut e: EntityCommands,
        fields: &Fields,
        sprites: &mut SystemParamItem<Self::Param>,
        _: QueryItem<Self::Data>,
    ) -> Result {
        let radius = fields.float("radius")?;
        let strength = fields.float("strength")?;
        let _level_target = fields.string("level_target").ok().to_owned();

        e.insert((
            Self {
                radius,
                gravity: strength * strength * radius,
                caster: Collider::circle(radius),
            },
            Collider::circle(8.),
            CollisionEventsEnabled,
            Animation::new(sprites.attractor_regular.clone_weak(), "anim"),
            AnimationMode::Repeat,
            EntityColor(Color::linear_rgba(1., 1., 12., 1.)),
            DebugRender::none(),
        ))
        .observe(
            |trigger: Trigger<OnCollisionStart>, mut commands: Commands| {
                if let Some(mut e) = trigger.body.and_then(|e| commands.get_entity(e).ok()) {
                    e.queue(TryHurt::by(trigger.target(), 1_000_000));
                }
            },
        );

        Ok(())
    }
}

#[derive(Debug, Clone, Component)]
pub struct AttractedPrediction {
    pub points: Vec<Vec2>,
    pub max_distance: f32,
}

#[derive(Debug, Copy, Clone, Default, Component)]
#[component(storage = "SparseSet")]
#[require(PenumbraEntity)]
pub struct AttractedInitial {
    pub ccw: bool,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Reflect, Actionlike, Serialize, Deserialize)]
pub enum AttractedAction {
    #[actionlike(Axis)]
    Accel,
    #[actionlike(Axis)]
    Hover,
    Precise,
    Parry,
}

#[derive(Debug, Clone, Component)]
#[require(ActionState<AttractedAction>)]
pub struct AttractedParams {
    pub ascend: f32,
    pub descend: f32,
    pub prograde: f32,
    pub retrograde: f32,
    pub precise_scale: f32,
}

pub fn detect_attracted_entities(
    pipeline: Res<SpatialQueryPipeline>,
    mut attractors: Query<(Entity, &Position, &Attractor, &mut AttractorEntities)>,
    mut penumbra_bodies: Query<
        (
            Entity,
            &Position,
            &mut LinearVelocity,
            Option<&mut LaunchTarget>,
            Option<&AttractedInitial>,
        ),
        (With<PenumbraEntity>, Without<NoAttract>),
    >,
    mut tmp: Local<Vec<Entity>>,
) {
    for (attractor_entity, &attractor_pos, attractor, mut attracted) in &mut attractors {
        tmp.clear();
        pipeline.shape_intersections_callback(
            &attractor.caster,
            *attractor_pos,
            0.,
            &SpatialQueryFilter::from_excluded_entities([attractor_entity]),
            |e| {
                if let Ok((e, &pos, mut linvel, target, initial)) = penumbra_bodies.get_mut(e) {
                    let r_vec = *attractor_pos - *pos;
                    let r = r_vec.length();

                    if let Some(&initial) = initial {
                        let initial_speed = (attractor.gravity / r).sqrt();
                        let initial_velocity = if initial.ccw {
                            -r_vec.perp()
                        } else {
                            r_vec.perp()
                        } / r
                            * initial_speed;
                        **linvel += initial_velocity;
                    }

                    if let Some(mut target) = target {
                        **target = Some(attractor_entity);
                    }

                    tmp.push(e);
                }

                true
            },
        );

        std::mem::swap(&mut **attracted, &mut *tmp);
    }
}

pub fn remove_attracted_initials(
    mut commands: Commands,
    query: Query<Entity, With<AttractedInitial>>,
) {
    for e in &query {
        commands.entity(e).remove::<AttractedInitial>();
    }
}

pub fn apply_attractor_accels(
    time: Res<Time<Substeps>>,
    attractors: Query<(&Position, &Attractor, &AttractorEntities)>,
    mut attracted: Query<(
        &Position,
        &mut SolverBody,
        Option<(&AttractedParams, &ActionState<AttractedAction>)>,
    )>,
) {
    let dt = time.delta_secs();
    for (&attractor_pos, attractor, attracted_entities) in &attractors {
        let mut attracted = attracted.iter_many_mut(&**attracted_entities);
        while let Some((&pos, body, hover)) = attracted.fetch_next() {
            let SolverBody {
                linear_velocity,
                angular_velocity,
                delta_position,
                ..
            } = body.into_inner();

            let (added_linvel, r_vec, r) = gravity_linvel(
                dt,
                *pos + *delta_position,
                *attractor_pos,
                attractor.gravity,
            );
            *linear_velocity += added_linvel;

            let crs = r_vec.x * linear_velocity.y - r_vec.y * linear_velocity.x;
            *angular_velocity = -(crs / (r * r));

            if let Some((param, state)) = hover {
                let precise_scale = if state.pressed(&AttractedAction::Precise) {
                    param.precise_scale
                } else {
                    1.
                };

                {
                    let prograde_value = state.clamped_value(&AttractedAction::Accel);

                    let r_vec = *linear_velocity;
                    let r = r_vec.length();
                    let prograde = if prograde_value > 0. {
                        param.prograde * prograde_value.min(1.)
                    } else {
                        param.retrograde * prograde_value.max(-1.)
                    } * precise_scale;

                    *linear_velocity += prograde / r * r_vec * dt;
                }

                {
                    let hover_value = state.clamped_value(&AttractedAction::Hover);

                    let r_vec = *attractor_pos - *pos;
                    let r = r_vec.length();
                    let hover = if hover_value > 0. {
                        param.ascend * (-hover_value).max(-1.)
                    } else {
                        param.descend * (-hover_value).min(1.)
                    } * precise_scale;

                    *linear_velocity += hover / r * r_vec * dt;
                }
            }
        }
    }
}

pub fn predict_attract_trajectory(
    time: Res<Time<Physics>>,
    attractors: Query<(&GlobalTransform, &Attractor, &Collider, &AttractorEntities)>,
    mut attracted: Query<(&GlobalTransform, &LinearVelocity, &mut AttractedPrediction)>,
) {
    let dt = time.delta_secs() / 3.;
    for (&attractor_trns, attractor, attractor_collision, attracted_entities) in &attractors {
        let g = attractor.gravity;
        let attractor_pos = attractor_trns.translation().truncate();
        let mut attracted = attracted.iter_many_mut(&**attracted_entities);

        'outer: while let Some((&trns, &vel, mut prediction)) = attracted.fetch_next() {
            let max = prediction.max_distance;
            let mut accum = 0.;

            let mut pos = trns.translation().truncate();
            let mut vel = *vel;

            prediction.points.clear();
            while accum < max {
                vel += gravity_linvel(dt, pos, attractor_pos, g).0;
                let new_pos = pos + vel * dt;

                accum += (new_pos - pos).length();
                pos = new_pos;

                if attractor_collision.contains_point(attractor_pos, 0., pos)
                    || !attractor.caster.contains_point(attractor_pos, 0., pos)
                {
                    continue 'outer;
                }

                prediction.points.push(pos);
            }
        }
    }
}

pub fn draw_attractor_radius(
    time: Res<Time>,
    sprites: Res<Sprites>,
    sprite_sections: Res<Assets<SpriteSection>>,
    attractors: Query<(&Attractor, &SpriteDrawer)>,
) {
    let [Some(ring_1), Some(ring_2), Some(ring_3), Some(ring_4)] = [
        sprite_sections.get(&sprites.ring_1),
        sprite_sections.get(&sprites.ring_2),
        sprite_sections.get(&sprites.ring_3),
        sprite_sections.get(&sprites.ring_4),
    ] else {
        return;
    };

    let elapsed = time.elapsed();
    let offset = Duration::from_millis(24);
    let bleed = 24;
    let lifetime = bleed * offset;

    for (attractor, drawer) in &attractors {
        let count = (2. * PI * attractor.radius / 8.).round().max(bleed as f32) as u32;
        let step = Rotation::radians(2. * PI / count as f32);
        let mut rotation = Rotation::IDENTITY;

        let total_lifetime = (count - bleed) * offset + lifetime;

        for i in 0..count {
            let elapsed = (elapsed + i * offset)
                .rem(total_lifetime)
                .min(lifetime)
                .div_duration_f32(lifetime);

            let (ring, alpha) = match elapsed {
                1. => (ring_1, 0.5),
                e if e >= 0.8 => (ring_2, 0.6),
                e if e >= 0.6 => (ring_3, 0.7),
                e if e >= 0.4 => (ring_4, 0.8),
                e if e >= 0.2 => (ring_3, 0.7),
                _ => (ring_2, 0.6),
            };

            let (sin, cos) = rotation.sin_cos();
            drawer.draw_at(
                vec3(cos * attractor.radius, sin * attractor.radius, 1.),
                Rot2::IDENTITY,
                ring.sprite_with(Color::linear_rgba(1., 2., 4., alpha), None, default()),
            );

            rotation *= step;
        }
    }
}

fn gravity_linvel(dt: f32, pos: Vec2, attractor_pos: Vec2, gravity: f32) -> (Vec2, Vec2, f32) {
    let r_vec = attractor_pos - pos;
    let r = r_vec.length();
    let linvel = gravity / (r * r * r) * r_vec * dt;

    (linvel, r_vec, r)
}
