use avian2d::{dynamics::integrator::IntegrationSet, prelude::*};
use bevy::prelude::*;
use leafwing_input_manager::prelude::*;

#[cfg(feature = "dev")]
use crate::logic::entities::penumbra::{draw_attract_trajectory, draw_attractor_radius};
use crate::logic::{
    LevelApp, LevelBounds, LevelUnload,
    entities::penumbra::{
        AttractedAction, Attractor, GenericPenumbra, SelenePenumbra, ThornPillar, ThornRing,
        apply_attractor_accels, detect_attracted_entities, predict_attract_trajectory,
        remove_attracted_initials, update_attracted_launching,
    },
};

pub mod penumbra;

#[derive(Debug, Copy, Clone, Component, Deref)]
#[component(immutable)]
pub struct Health(pub u32);
impl Health {
    pub fn new(amount: u32) -> Self {
        if amount == 0 {
            warn!("`Health` shouldn't be created with 0.");
        }
        Self(amount.max(1))
    }

    pub fn hurt(&mut self, amount: u32) -> bool {
        self.change(-(amount as i32), None)
    }

    pub fn change(&mut self, delta: i32, max: Option<MaxHealth>) -> bool {
        if self.0 > 0 {
            self.0 = self
                .saturating_add_signed(delta)
                .min(max.as_deref().copied().unwrap_or(u32::MAX));
            self.0 == 0
        } else {
            false
        }
    }
}

#[derive(Debug, Copy, Clone, Component, Deref)]
#[component(immutable)]
pub struct MaxHealth(pub u32);
impl MaxHealth {
    pub fn new(amount: u32) -> Self {
        if amount == 0 {
            warn!("`MaxHealth` shouldn't be created with 0.");
        }
        Self(amount.max(1))
    }
}

#[derive(Debug, Copy, Clone, Event)]
pub struct TryHurt {
    pub by: Entity,
    pub amount: u32,
    pub stopped: bool,
}

impl TryHurt {
    pub fn new(amount: u32) -> Self {
        Self::by(Entity::PLACEHOLDER, amount)
    }

    pub fn by(by: Entity, amount: u32) -> Self {
        Self {
            by,
            amount,
            stopped: false,
        }
    }
}

impl EntityCommand<Result> for TryHurt {
    fn apply(mut self, mut entity: EntityWorldMut) -> Result {
        let id = entity.id();
        entity.world_scope(|world| world.trigger_targets_ref(&mut self, id));

        if !self.stopped {
            let Some(should_kill) =
                entity.modify_component(|health: &mut Health| health.hurt(self.amount))
            else {
                return Ok(());
            };

            entity.trigger(Hurt::by(self.by, self.amount));
            if should_kill {
                entity.trigger(Killed::by(self.by));
                if !entity.contains::<NoKillDespawn>() {
                    entity.despawn();
                }
            }
        }

        Ok(())
    }
}

#[derive(Debug, Copy, Clone, Event)]
pub struct Hurt {
    pub by: Entity,
    pub amount: u32,
}

impl Hurt {
    pub fn new(amount: u32) -> Self {
        Self::by(Entity::PLACEHOLDER, amount)
    }

    pub fn by(by: Entity, amount: u32) -> Self {
        Self { by, amount }
    }
}

#[derive(Debug, Copy, Clone, Event)]
pub struct Killed {
    pub by: Entity,
}

impl Killed {
    pub fn new() -> Self {
        Self::by(Entity::PLACEHOLDER)
    }

    pub fn by(by: Entity) -> Self {
        Self { by }
    }
}

#[derive(Debug, Copy, Clone, Default, Component)]
pub struct NoKillDespawn;

pub fn kill_out_of_bounds(
    commands: ParallelCommands,
    level_bounds: Query<&LevelBounds, Without<LevelUnload>>,
    entities: Query<(Entity, &Position, Has<NoKillDespawn>)>,
) {
    let Ok(&level_bounds) = level_bounds.single() else {
        return;
    };

    entities.par_iter().for_each(|(e, &pos, no_kill_despawn)| {
        if pos.x < 0. || pos.x > level_bounds.x || pos.y < 0. || pos.y > level_bounds.x {
            commands.command_scope(|mut commands| {
                let mut e = commands.entity(e);
                e.trigger(Killed::new());

                if !no_kill_despawn {
                    e.despawn();
                }
            });
        }
    });
}

#[derive(Debug, Copy, Clone, Default)]
pub struct EntitiesPlugin;
impl Plugin for EntitiesPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(InputManagerPlugin::<AttractedAction>::default())
            .register_level_entity::<Attractor>("attractor")
            .register_level_entity::<SelenePenumbra>("selene_penumbra")
            .register_level_entity::<GenericPenumbra>("generic_penumbra")
            .register_level_entity::<ThornPillar>("thorn_pillar")
            .register_level_entity::<ThornRing>("thorn_ring")
            .add_systems(Update, update_attracted_launching)
            .add_systems(
                SubstepSchedule,
                apply_attractor_accels
                    .in_set(IntegrationSet::Velocity)
                    .ambiguous_with_all(),
            )
            .add_systems(
                PhysicsSchedule,
                (
                    predict_attract_trajectory.after(SolverSet::Finalize),
                    (
                        (detect_attracted_entities, remove_attracted_initials).chain(),
                        kill_out_of_bounds,
                    )
                        .in_set(PhysicsStepSet::SpatialQuery)
                        .after(update_spatial_query_pipeline),
                )
                    .ambiguous_with_all(),
            );

        #[cfg(feature = "dev")]
        app.add_systems(Update, (draw_attractor_radius, draw_attract_trajectory));
    }
}
