use std::time::Duration;

use bevy::{
    ecs::{component::HookContext, world::DeferredWorld},
    prelude::*,
};

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
