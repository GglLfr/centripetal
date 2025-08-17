use crate::{
    Affected, IntoResultSystem, Observed, despawn,
    graphics::{Animation, SpriteSheet},
    logic::entities::TryHurt,
    math::FloatTransformExt,
    prelude::*,
};

#[derive(Debug, Copy, Clone, Component)]
pub struct Timed {
    pub lifetime: Duration,
    life: Duration,
    frac_f64: f64,
    frac: f32,
    finished: bool,
}

impl Timed {
    pub fn new(lifetime: Duration) -> Self {
        Self {
            lifetime: lifetime.max(Duration::from_nanos(1)),
            life: Duration::ZERO,
            frac_f64: 0.,
            frac: 0.,
            finished: false,
        }
    }

    pub fn new_at(lifetime: Duration, life: Duration) -> Self {
        let lifetime = lifetime.max(Duration::from_nanos(1));
        let life = life.min(lifetime);
        let frac = life.div_duration_f64(lifetime);
        Self {
            lifetime,
            life,
            frac_f64: frac,
            frac: frac as f32,
            finished: false,
        }
    }

    pub fn from_anim(key: impl Into<Cow<'static, str>>) -> impl Bundle {
        let key = key.into();
        (
            // Avoid division by zero.
            Timed::new(Duration::from_nanos(1)),
            Affected::by(
                move |In(e): In<Entity>, mut query: Query<(&mut Self, &Animation)>, sheets: Res<Assets<SpriteSheet>>| {
                    let Ok((mut this, anim)) = query.get_mut(e) else { return };
                    let Some(sheet) = sheets.get(&anim.sprite) else { return };
                    let Some(range) = sheet.tags.get(&*key).cloned() else { return };

                    let mut lifetime = Duration::ZERO;
                    for i in range {
                        let Some(&duration) = sheet.durations.get(i) else { continue };
                        lifetime += duration;
                    }

                    this.lifetime = lifetime.max(Duration::from_nanos(1));
                },
            ),
        )
    }

    pub fn run<M>(lifetime: Duration, sys: impl IntoResultSystem<(), (), M>) -> impl Bundle {
        let mut sys = IntoResultSystem::into_system(sys);
        (
            Self::new(lifetime),
            Observed::by(move |trigger: Trigger<TimeFinished>, world: &mut World| -> Result {
                sys.initialize(world);
                sys.validate_param(world)?;
                sys.run_without_applying_deferred((), world)?;

                let mut world = DeferredWorld::from(world);
                sys.queue_deferred(world.reborrow());

                world.commands().queue(despawn(trigger.target()));
                Ok(())
            }),
        )
    }

    pub fn repeat<M>(lifetime: Duration, sys: impl IntoResultSystem<In<Entity>, (), M>) -> impl Bundle {
        let mut sys = IntoResultSystem::into_system(sys);
        let mut initialized = false;

        (
            Self::new(lifetime),
            Observed::by(
                move |trigger: Trigger<TimeFinished>, world: &mut World, query: &mut QueryState<&mut Self>| -> Result {
                    if !std::mem::replace(&mut initialized, true) {
                        sys.initialize(world);
                    }

                    sys.validate_param(world)?;
                    sys.run_without_applying_deferred(trigger.target(), world)?;
                    sys.queue_deferred(DeferredWorld::from(&mut *world));

                    let mut timed = query.get_mut(world, trigger.target())?;
                    timed.life = trigger.overtime;
                    timed.frac_f64 = trigger.overtime_frac_f64;
                    timed.frac = trigger.overtime_frac;
                    timed.finished = false;

                    Ok(())
                },
            ),
        )
    }

    pub fn kill_on_finished(trigger: Trigger<TimeFinished>, world: &mut World) {
        world.trigger_targets(TryHurt::by(trigger.target(), i32::MAX as u32), trigger.target());
    }

    pub fn despawn_on_finished(trigger: Trigger<TimeFinished>, mut commands: Commands) {
        commands.queue(despawn(trigger.target()));
    }

    pub fn life(&self) -> Duration {
        self.life
    }

    pub fn frac_f64(&self) -> f64 {
        self.frac_f64
    }

    pub fn frac(&self) -> f32 {
        self.frac
    }
}

#[derive(Debug, Copy, Clone, Default, Event)]
pub struct TimeFinished {
    pub count: usize,
    pub overtime: Duration,
    pub overtime_frac_f64: f64,
    pub overtime_frac: f32,
}

pub fn update_timed(commands: ParallelCommands, time: Res<Time>, mut timed_query: Query<(Entity, &mut Timed)>) {
    let delta = time.delta();
    timed_query.par_iter_mut().for_each(|(e, mut timed)| {
        let lifetime = timed.lifetime;
        timed.life += delta;

        if timed.life < lifetime {
            let frac = timed.life.div_duration_f64(lifetime);
            timed.frac_f64 = frac;
            timed.frac = frac as f32;
        } else if !std::mem::replace(&mut timed.finished, true) {
            timed.frac_f64 = 1.;
            timed.frac = 1.;

            let mut count = 0;
            let mut overtime = std::mem::replace(&mut timed.life, lifetime);

            while overtime >= lifetime {
                count += 1;
                overtime -= lifetime;
            }

            let frac = overtime.div_duration_f64(lifetime);
            commands.command_scope(|mut commands| {
                commands.trigger_targets(
                    TimeFinished {
                        count,
                        overtime,
                        overtime_frac_f64: frac,
                        overtime_frac: frac as f32,
                    },
                    e,
                );
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

pub fn update_time_stun(time: Res<Time<Real>>, mut virtual_time: ResMut<Time<Virtual>>, mut commands: Commands, stuns: Query<(Entity, &TimeStun)>) {
    let now = time.elapsed();
    let mut scale = 1.;

    for (e, &TimeStun(kind, started)) in &stuns {
        scale = match kind {
            TimeStunKind::ShortInstant => {
                if now - started >= Duration::from_millis(200) {
                    commands.queue(despawn(e));
                    1.
                } else {
                    0.075
                }
            }
            TimeStunKind::LongSmooth => {
                if now - started >= Duration::from_millis(1000) {
                    commands.queue(despawn(e));
                    1.
                } else {
                    let f = (now - started).div_duration_f32(Duration::from_millis(1000));
                    if f < 0.15 { 0.075 } else { 0.2 + f.threshold(0.2, 1.) * 0.8 }
                }
            }
        }
        .min(scale);
    }

    virtual_time.set_relative_speed(scale);
}
