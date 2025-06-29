use avian2d::prelude::*;
use bevy::{
    ecs::{entity::MapEntities, query::QueryItem, system::SystemParamItem},
    prelude::*,
};

use crate::logic::{FromLevelEntity, LevelEntity, PenumbraEntity};

#[derive(Debug, Clone, Component)]
#[require(PenumbraEntity, AttractedEntities)]
pub struct Attractor {
    pub radius: f32,
    pub gravity: f32,
    pub caster: Collider,
}

#[derive(Debug, Copy, Clone, Default, Component)]
#[component(storage = "SparseSet")]
#[require(PenumbraEntity)]
pub struct AttractorInitial {
    pub ccw: bool,
}

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

        Ok(())
    }
}

#[derive(Debug, Clone, Default, Component, MapEntities, Deref, DerefMut)]
pub struct AttractedEntities(#[entities] pub Vec<Entity>);

pub fn draw_attractor_radius(mut gizmos: Gizmos, attractors: Query<(&Position, &Attractor)>) {
    for (&pos, attractor) in &attractors {
        gizmos.circle_2d(
            Isometry2d::from_translation(*pos),
            attractor.radius,
            LinearRgba::new(0.67, 0.67, 0.67, 0.67),
        );
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
    mut attracted: Query<(&Position, &AccumulatedTranslation, &mut LinearVelocity)>,
) {
    let dt = time.delta_secs();
    for (&attractor_pos, attractor, attracted_entities) in &attractors {
        let mut attracted = attracted.iter_many_mut(&**attracted_entities);

        while let Some((&pos, &accum_pos, mut vel)) = attracted.fetch_next() {
            let pos = *pos + *accum_pos;
            let r_vec = *attractor_pos - pos;
            let r = r_vec.length();

            let gravity_accel = attractor.gravity / (r * r * r) * r_vec;
            **vel += gravity_accel * dt;
        }
    }
}
