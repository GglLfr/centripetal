use std::time::Duration;

use bevy::{
    ecs::{component::HookContext, world::DeferredWorld},
    prelude::*,
};

use crate::{IntoResultSystem, Observed, math::FloatTransformExt};

#[derive(Debug, Copy, Clone, Component)]
#[component(on_insert = on_timed_insert)]
pub struct Timed {
    pub lifetime: Duration,
    started: Duration,
    elapsed: Duration,
    frac_f64: f64,
    frac: f32,
    finished: bool,
}

impl Timed {
    pub fn new(lifetime: Duration) -> Self {
        Self {
            lifetime,
            started: Duration::ZERO,
            elapsed: Duration::ZERO,
            frac_f64: 0.,
            frac: 0.,
            finished: false,
        }
    }

    pub fn run<M>(lifetime: Duration, sys: impl IntoResultSystem<(), (), M>) -> impl Bundle {
        let mut sys = IntoResultSystem::into_result_system(sys);
        (
            Self::new(lifetime),
            Observed::by(
                move |trigger: Trigger<OnTimeFinished>, world: &mut World| -> Result {
                    sys.initialize(world);
                    sys.validate_param(world)?;
                    sys.run_without_applying_deferred((), world)?;

                    let mut world = DeferredWorld::from(world);
                    sys.queue_deferred(world.reborrow());
                    world
                        .reborrow()
                        .commands()
                        .entity(trigger.target())
                        .despawn();

                    Ok(())
                },
            ),
        )
    }

    pub fn despawn_on_finished(trigger: Trigger<OnTimeFinished>, mut commands: Commands) {
        if let Ok(mut e) = commands.get_entity(trigger.target()) {
            e.despawn();
        }
    }

    pub fn started(&self) -> Duration {
        self.started
    }

    pub fn elapsed(&self) -> Duration {
        self.elapsed
    }

    pub fn frac_f64(&self) -> f64 {
        self.frac_f64
    }

    pub fn frac(&self) -> f32 {
        self.frac
    }
}

#[derive(Debug, Copy, Clone, Default, Event)]
pub struct OnTimeFinished;

fn on_timed_insert(mut world: DeferredWorld, HookContext { entity, .. }: HookContext) {
    let now = world.resource::<Time>().elapsed();
    world.entity_mut(entity).get_mut::<Timed>().unwrap().started = now;
}

pub fn update_timed(
    commands: ParallelCommands,
    time: Res<Time>,
    mut timed_query: Query<(Entity, &mut Timed)>,
) {
    let now = time.elapsed();
    timed_query.par_iter_mut().for_each(|(e, mut timed)| {
        let elapsed = (now - timed.started).min(timed.lifetime);
        let frac = elapsed.div_duration_f64(timed.lifetime);

        timed.elapsed = elapsed;
        timed.frac_f64 = frac;
        timed.frac = frac as f32;

        if elapsed == timed.lifetime && !std::mem::replace(&mut timed.finished, true) {
            commands.command_scope(|mut commands| {
                commands.entity(e).trigger(OnTimeFinished);
            });
        }
    });
}

#[derive(Debug, Copy, Clone, Component)]
#[component(on_insert = on_time_stun_insert)]
pub struct TimeStun(TimeStunKind, Duration);
impl TimeStun {
    pub fn new(kind: TimeStunKind) -> Self {
        Self(kind, Duration::ZERO)
    }

    pub fn short_instant() -> Self {
        Self::new(TimeStunKind::ShortInstant)
    }

    pub fn long_smooth() -> Self {
        Self::new(TimeStunKind::LongSmooth)
    }
}

#[derive(Debug, Copy, Clone, Default)]
pub enum TimeStunKind {
    #[default]
    ShortInstant,
    LongSmooth,
}

fn on_time_stun_insert(mut world: DeferredWorld, HookContext { entity, .. }: HookContext) {
    let elapsed = world.resource::<Time<Real>>().elapsed();
    world.entity_mut(entity).get_mut::<TimeStun>().unwrap().1 = elapsed;
}

pub fn update_time_stun(
    time: Res<Time<Real>>,
    mut virtual_time: ResMut<Time<Virtual>>,
    mut commands: Commands,
    stuns: Query<(Entity, &TimeStun)>,
) {
    let now = time.elapsed();
    let mut scale = 1.;

    for (e, &TimeStun(kind, started)) in &stuns {
        scale = match kind {
            TimeStunKind::ShortInstant => {
                if now - started >= Duration::from_millis(100) {
                    commands.entity(e).despawn();
                    1.
                } else {
                    0.
                }
            }
            TimeStunKind::LongSmooth => {
                if now - started >= Duration::from_millis(1000) {
                    commands.entity(e).despawn();
                    1.
                } else {
                    let f = (now - started).div_duration_f32(Duration::from_millis(1000));
                    if f < 0.1 {
                        0.
                    } else {
                        0.2 + f.threshold(0.1, 1.) * 0.8
                    }
                }
            }
        }
        .min(scale);
    }

    virtual_time.set_relative_speed(scale);
}
