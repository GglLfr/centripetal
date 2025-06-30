use avian2d::prelude::*;
use bevy::{
    ecs::{entity::MapEntities, query::QueryItem, system::SystemParamItem},
    math::VectorSpace,
    prelude::*,
};
use leafwing_input_manager::prelude::*;

use crate::logic::{FromLevelEntity, LevelEntity, PenumbraEntity};

#[derive(Debug, Clone, Component)]
#[require(PenumbraEntity, AttractedEntities)]
pub struct Attractor {
    pub radius: f32,
    pub gravity: f32,
    pub caster: Collider,
}

#[derive(Debug, Clone, Default, Component, MapEntities, Deref, DerefMut)]
pub struct AttractedEntities(#[entities] pub Vec<Entity>);

impl FromLevelEntity for Attractor {
    type Param = ();
    type Data = ();

    fn from_level_entity(
        mut e: EntityCommands,
        entity: &LevelEntity,
        _: &mut SystemParamItem<Self::Param>,
        _: QueryItem<Self::Data>,
    ) -> Result {
        let radius = entity.float("radius")?;
        let strength = entity.float("strength")?;
        let _level_target = entity.string("level_target").ok().to_owned();

        e.insert((
            Self {
                radius,
                gravity: strength * strength * radius,
                caster: Collider::circle(radius),
            },
            Collider::circle(8.),
        ));

        debug!("Spawned attractor {}!", e.id());
        Ok(())
    }
}

#[derive(Debug, Clone, Component)]
pub struct AttractorPrediction {
    pub points: Vec<Vec2>,
    pub max_distance: f32,
}

#[derive(Debug, Copy, Clone, Default, Component)]
#[component(storage = "SparseSet")]
#[require(PenumbraEntity)]
pub struct AttractorInitial {
    pub ccw: bool,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Reflect, Actionlike)]
pub enum AttractorHoverAction {
    #[actionlike(Axis)]
    Prograde,
    #[actionlike(Axis)]
    Hover,
}

#[derive(Debug, Copy, Clone, Component)]
#[require(ActionState<AttractorHoverAction>)]
pub struct AttractorHoverParams {
    pub centrifugal: f32,
    pub centripetal: f32,
    pub prograde: f32,
    pub retrograde: f32,
}

pub fn draw_attractor_radius(mut gizmos: Gizmos, attractors: Query<(&Position, &Attractor)>) {
    for (&pos, attractor) in &attractors {
        gizmos
            .circle_2d(
                Isometry2d::from_translation(*pos),
                attractor.radius,
                LinearRgba::new(0.67, 0.67, 0.67, 0.67),
            )
            .resolution(64);
    }
}

pub fn detect_attracted_entities(
    mut commands: Commands,
    pipeline: Res<SpatialQueryPipeline>,
    mut attractors: Query<(Entity, &Position, &Attractor, &mut AttractedEntities)>,
    mut penumbra_bodies: Query<(Entity, &Position, &mut LinearVelocity, Option<&AttractorInitial>), With<PenumbraEntity>>,
    mut tmp: Local<Vec<Entity>>,
) {
    for (attractor_entity, &attractor_pos, attractor, mut attracted) in &mut attractors {
        tmp.clear();
        pipeline.shape_intersections_callback(&attractor.caster, *attractor_pos, 0., &SpatialQueryFilter::DEFAULT, |e| {
            if e != attractor_entity &&
                let Ok((e, &pos, mut vel, initial)) = penumbra_bodies.get_mut(e)
            {
                if let Some(&initial) = initial {
                    let r_vec = *attractor_pos - *pos;
                    let r = r_vec.length();

                    let initial_speed = (attractor.gravity / r).sqrt();
                    let initial_velocity = if initial.ccw { -r_vec.perp() } else { r_vec.perp() } / r * initial_speed;
                    **vel += initial_velocity;

                    commands.entity(e).remove::<AttractorInitial>();
                }

                tmp.push(e);
            }

            true
        });

        std::mem::swap(&mut **attracted, &mut *tmp);
    }
}

pub fn apply_attractor_accels(
    time: Res<Time<Substeps>>,
    attractors: Query<(&Position, &Attractor, &AttractedEntities)>,
    mut attracted: Query<(
        &Position,
        &AccumulatedTranslation,
        Option<(&AttractorHoverParams, &ActionState<AttractorHoverAction>)>,
        &mut LinearVelocity,
    )>,
) {
    let dt = time.delta_secs();
    for (&attractor_pos, attractor, attracted_entities) in &attractors {
        let mut attracted = attracted.iter_many_mut(&**attracted_entities);
        while let Some((&pos, &accum_pos, hover, mut vel)) = attracted.fetch_next() {
            **vel += gravity_accel(*pos + *accum_pos, *attractor_pos, attractor.gravity) * dt;
            if let Some((&param, state)) = hover {
                if let Some(axis) = state.axis_data(&AttractorHoverAction::Prograde) &&
                    axis.value.abs() >= 0.01
                {
                    let r_vec = **vel;
                    let r = r_vec.length();
                    let prograde = if axis.value > 0. {
                        param.prograde * axis.value.min(1.)
                    } else {
                        param.retrograde * axis.value.max(-1.)
                    };

                    **vel += prograde / r * r_vec * dt;
                }

                if let Some(axis) = state.axis_data(&AttractorHoverAction::Hover) &&
                    axis.value.abs() >= 0.01
                {
                    let r_vec = *attractor_pos - *pos;
                    let r = r_vec.length();
                    let hover = if axis.value > 0. {
                        param.centrifugal * (-axis.value).max(-1.)
                    } else {
                        param.centripetal * (-axis.value).min(1.)
                    };

                    **vel += hover / r * r_vec * dt;
                }
            }
        }
    }
}

pub fn predict_attract_trajectory(
    time: Res<Time<Physics>>,
    attractors: Query<(&Position, &Attractor, &Collider, &AttractedEntities)>,
    mut attracted: Query<(&Position, &LinearVelocity, &mut AttractorPrediction)>,
) {
    let dt = time.delta_secs() / 3.;
    for (&attractor_pos, attractor, attractor_collision, attracted_entities) in &attractors {
        let g = attractor.gravity;
        let mut attracted = attracted.iter_many_mut(&**attracted_entities);

        'outer: while let Some((&pos, &vel, mut prediction)) = attracted.fetch_next() {
            let max = prediction.max_distance;
            let mut accum = 0.;

            let mut pos = *pos;
            let mut vel = *vel;

            prediction.points.clear();
            while accum < max {
                vel += gravity_accel(pos, *attractor_pos, g) * dt;
                let new_pos = pos + vel * dt;

                accum += (new_pos - pos).length();
                pos = new_pos;

                if attractor_collision.contains_point(attractor_pos, 0., pos) ||
                    !attractor.caster.contains_point(attractor_pos, 0., pos)
                {
                    continue 'outer
                }

                prediction.points.push(pos);
            }
        }
    }
}

pub fn draw_attract_trajectory(mut gizmos: Gizmos, attracted: Query<(&Position, &AttractorPrediction)>) {
    for (&pos, prediction) in &attracted {
        let mut pos = *pos;
        for (i, &point) in prediction.points.iter().enumerate() {
            gizmos.line_2d(
                pos,
                point,
                LinearRgba::WHITE.lerp(LinearRgba::NONE, i as f32 / (prediction.points.len() - 1) as f32),
            );
            pos = point;
        }
    }
}

fn gravity_accel(pos: Vec2, attractor_pos: Vec2, gravity: f32) -> Vec2 {
    let r_vec = attractor_pos - pos;
    let r = r_vec.length();
    gravity / (r * r * r) * r_vec
}
