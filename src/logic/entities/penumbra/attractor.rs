use std::time::Duration;

use avian2d::{dynamics::solver::solver_body::SolverBody, prelude::*};
use bevy::{
    ecs::{
        query::QueryItem,
        system::{IntoObserverSystem, ObserverSystem, SystemParamItem},
    },
    math::{FloatOrd, VectorSpace},
    prelude::*,
};
use leafwing_input_manager::{buttonlike::ButtonState, prelude::*};
use serde::{Deserialize, Serialize};

use crate::logic::{Fields, FromLevelEntity, entities::penumbra::PenumbraEntity};

#[derive(Debug, Clone, Component)]
#[require(PenumbraEntity, AttractorEntities)]
pub struct Attractor {
    pub radius: f32,
    pub gravity: f32,
    pub caster: Collider,
}

impl Attractor {
    pub fn bundle(radius: f32, strength: f32) -> impl Bundle {
        (
            Self {
                radius,
                gravity: strength * strength * radius,
                caster: Collider::circle(radius),
            },
            Collider::circle(8.),
        )
    }
}

#[derive(Debug, Clone, Default, Component, Deref, DerefMut)]
pub struct AttractorEntities(pub Vec<Entity>);

impl FromLevelEntity for Attractor {
    type Param = ();
    type Data = ();

    fn from_level_entity(
        mut e: EntityCommands,
        fields: &Fields,
        _: &mut SystemParamItem<Self::Param>,
        _: QueryItem<Self::Data>,
    ) -> Result {
        let radius = fields.float("radius")?;
        let strength = fields.float("strength")?;
        let _level_target = fields.string("level_target").ok().to_owned();

        e.insert(Self::bundle(radius, strength));

        debug!("Spawned attractor {}!", e.id());
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
    Prograde,
    #[actionlike(Axis)]
    Hover,
    Precise,
    Launch,
    Parry,
}

#[derive(Debug, Clone, Component)]
#[require(ActionState<AttractedAction>, AttractedLaunching)]
pub struct AttractedParams {
    pub ascend: f32,
    pub descend: f32,
    pub prograde: f32,
    pub retrograde: f32,
    pub precise_scale: f32,
    pub launches: Vec<AttractedLaunch>,
    pub launch_cooldown: Duration,
}

#[derive(Debug, Clone, Component)]
#[component(immutable)]
pub enum AttractedLaunching {
    Idle { last_launched: Duration },
    Charging { started: Duration },
    Launch { target: AttractedLaunch },
}

impl Default for AttractedLaunching {
    fn default() -> Self {
        Self::Idle {
            last_launched: Duration::ZERO,
        }
    }
}

#[derive(Debug, Clone)]
pub struct AttractedLaunch {
    pub charge: Duration,
    pub damage: u32,
}

#[derive(Debug, Clone)]
pub struct LaunchCommand {
    pub target: AttractedLaunch,
    pub launcher_entity: Entity,
    pub launcher_pos: Vec2,
    pub attractor_entity: Entity,
    pub attractor_pos: Vec2,
    hits: Vec<RayHitData>,
}

impl EntityCommand for LaunchCommand {
    fn apply(mut self, mut entity: EntityWorldMut) {
        let hits = std::mem::take(&mut self.hits);
        let mut trigger = OnLaunch {
            command: self,
            hit: None,
            stopped: false,
        };

        entity.world_scope(|world| {
            for (i, &hit) in hits.iter().enumerate() {
                trigger.hit = Some((i, hit));
                world.trigger_targets_ref(&mut trigger, hit.entity);

                if trigger.stopped {
                    return
                }
            }
        });

        if !trigger.stopped {
            entity.trigger(trigger);
        }
    }
}

#[derive(Debug, Clone, Event, Deref)]
pub struct OnLaunch {
    #[deref]
    command: LaunchCommand,
    pub hit: Option<(usize, RayHitData)>,
    pub stopped: bool,
}

impl OnLaunch {
    pub fn collide(
        stop: bool,
        mut apply: impl FnMut(EntityCommands, Entity) + 'static + Send + Sync,
    ) -> impl ObserverSystem<Self, ()> {
        IntoObserverSystem::<Self, (), _>::into_system(move |mut trigger: Trigger<Self>, mut commands: Commands| {
            if stop {
                trigger.event_mut().stopped = true;
            }

            apply(commands.entity(trigger.event().command.launcher_entity), trigger.target());
        })
    }
}

pub fn update_attracted_launching(
    mut commands: Commands,
    time: Res<Time>,
    launches: Query<(Entity, &AttractedParams, &ActionState<AttractedAction>, &AttractedLaunching)>,
) {
    let now = time.elapsed();
    for (e, param, state, launching) in &launches {
        if param.launches.is_empty() {
            continue
        }

        let Some(data) = state.button_data(&AttractedAction::Launch) else { continue };
        let target = if let &AttractedLaunching::Charging { started } = launching {
            let mut duration = now - started;
            let mut target = None;
            let mut i = 0;

            while let Some(launch) = param.launches.get(i) &&
                launch.charge <= duration
            {
                i += 1;
                duration -= launch.charge;
                target = Some(launch);
            }

            let overcharged = param.launches.get(i + 1).is_none() && !duration.is_zero();
            target.cloned().zip(Some(overcharged))
        } else {
            None
        };

        commands.entity(e).insert(
            match (
                if state.action_disabled(&AttractedAction::Launch) { ButtonState::Released } else { data.state },
                target,
                launching,
            ) {
                (ButtonState::JustPressed, None, AttractedLaunching::Idle { last_launched: last })
                    if now - *last >= param.launch_cooldown =>
                {
                    AttractedLaunching::Charging { started: now }
                }
                (ButtonState::Pressed, Some((target, true)), AttractedLaunching::Charging { .. }) |
                (ButtonState::JustReleased, Some((target, ..)), AttractedLaunching::Charging { .. }) => {
                    AttractedLaunching::Launch { target }
                }
                (ButtonState::Released, Some(..), AttractedLaunching::Charging { .. }) |
                (ButtonState::JustReleased, .., AttractedLaunching::Charging { .. }) => {
                    AttractedLaunching::Idle { last_launched: now }
                }
                _ => continue,
            },
        );
    }
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
    time: Res<Time>,
    pipeline: Res<SpatialQueryPipeline>,
    mut attractors: Query<(Entity, &Position, &Attractor, &mut AttractorEntities)>,
    mut penumbra_bodies: Query<
        (
            Entity,
            &Position,
            &mut LinearVelocity,
            Option<&AttractedLaunching>,
            Option<&AttractedInitial>,
        ),
        With<PenumbraEntity>,
    >,
    mut tmp: Local<Vec<Entity>>,
) {
    let now = time.elapsed();
    for (attractor_entity, &attractor_pos, attractor, mut attracted) in &mut attractors {
        tmp.clear();
        pipeline.shape_intersections_callback(
            &attractor.caster,
            *attractor_pos,
            0.,
            &SpatialQueryFilter::from_excluded_entities([attractor_entity]),
            |e| {
                if let Ok((e, &pos, mut linvel, launching, initial)) = penumbra_bodies.get_mut(e) {
                    let r_vec = *attractor_pos - *pos;
                    let r = r_vec.length();

                    if let Some(&initial) = initial {
                        let initial_speed = (attractor.gravity / r).sqrt();
                        let initial_velocity = if initial.ccw { -r_vec.perp() } else { r_vec.perp() } / r * initial_speed;
                        **linvel += initial_velocity;
                    }

                    if let Some(AttractedLaunching::Launch { target }) = launching {
                        commands.entity(e).insert(AttractedLaunching::Idle { last_launched: now });

                        let Ok(dir) = Dir2::new(r_vec) else { return true };
                        let mut hits = pipeline.ray_hits(*pos, dir, r, u32::MAX, true, &SpatialQueryFilter {
                            excluded_entities: [e, attractor_entity].into_iter().collect(),
                            ..SpatialQueryFilter::DEFAULT
                        });

                        hits.sort_unstable_by_key(|data| FloatOrd(data.distance));
                        commands.entity(e).queue(LaunchCommand {
                            target: target.clone(),
                            launcher_entity: e,
                            launcher_pos: *pos,
                            attractor_entity,
                            attractor_pos: *attractor_pos,
                            hits,
                        });
                    }

                    tmp.push(e);
                }

                true
            },
        );

        std::mem::swap(&mut **attracted, &mut *tmp);
    }
}

pub fn remove_attracted_initials(mut commands: Commands, query: Query<Entity, With<AttractedInitial>>) {
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
        Option<(&AttractedParams, &AttractedLaunching, &ActionState<AttractedAction>)>,
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

            let (added_linvel, r_vec, r) = gravity_linvel(dt, *pos + *delta_position, *attractor_pos, attractor.gravity);
            *linear_velocity += added_linvel;

            let crs = r_vec.x * linear_velocity.y - r_vec.y * linear_velocity.x;
            *angular_velocity = -(crs / (r * r));

            if let Some((param, AttractedLaunching::Idle { .. }, state)) = hover {
                let precise_scale = if state.pressed(&AttractedAction::Precise) { param.precise_scale } else { 1. };

                {
                    let prograde_value = state.clamped_value(&AttractedAction::Prograde);

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
    attractors: Query<(&Position, &Attractor, &Collider, &AttractorEntities)>,
    mut attracted: Query<(&Position, &LinearVelocity, &mut AttractedPrediction)>,
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
                vel += gravity_linvel(dt, pos, *attractor_pos, g).0;
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

pub fn draw_attract_trajectory(mut gizmos: Gizmos, attracted: Query<(&Position, &AttractedPrediction)>) {
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

fn gravity_linvel(dt: f32, pos: Vec2, attractor_pos: Vec2, gravity: f32) -> (Vec2, Vec2, f32) {
    let r_vec = attractor_pos - pos;
    let r = r_vec.length();
    let linvel = gravity / (r * r * r) * r_vec * dt;

    (linvel, r_vec, r)
}
