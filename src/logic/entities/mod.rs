use avian2d::dynamics::integrator::IntegrationSet;

#[cfg(feature = "dev")]
use crate::logic::entities::penumbra::draw_attractor_radius;
use crate::{
    logic::{
        GameState, LevelApp, LevelBounds, LevelUnload,
        entities::penumbra::{
            AttractedAction, Attractor, GenericPenumbra, LaunchAction, SelenePenumbra, ThornPillar,
            ThornRing, apply_attractor_accels, apply_homing_velocity, color_selene_hurt,
            color_selene_slash, detect_attracted_entities, draw_selene_launch_disc,
            draw_selene_prediction_trajectory, predict_attract_trajectory,
            remove_attracted_initials, trigger_launch_charging, update_launch_charging,
            update_launch_idle,
        },
    },
    prelude::*,
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
    entities: Query<(Entity, &Position)>,
) {
    let Ok(&level_bounds) = level_bounds.single() else {
        return;
    };

    entities.par_iter().for_each(|(e, &pos)| {
        if pos.x < 0. || pos.x > level_bounds.x || pos.y < 0. || pos.y > level_bounds.x {
            commands.command_scope(|mut commands| {
                commands.entity(e).queue(TryHurt::new(i32::MAX as u32));
            });
        }
    });
}

#[derive(Debug, Copy, Clone, Default)]
pub struct EntitiesPlugin;
impl Plugin for EntitiesPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            InputManagerPlugin::<AttractedAction>::default(),
            InputManagerPlugin::<LaunchAction>::default(),
        ))
        .register_level_entity::<Attractor>("attractor")
        .register_level_entity::<SelenePenumbra>("selene_penumbra")
        .register_level_entity::<GenericPenumbra>("generic_penumbra")
        .register_level_entity::<ThornPillar>("thorn_pillar")
        .register_level_entity::<ThornRing>("thorn_ring")
        .add_systems(
            Update,
            (
                color_selene_hurt,
                color_selene_slash,
                update_launch_idle,
                update_launch_charging,
                draw_selene_launch_disc,
                draw_attractor_radius,
            )
                .run_if(in_state(GameState::InGame)),
        )
        .add_systems(
            PostUpdate,
            draw_selene_prediction_trajectory
                .run_if(in_state(GameState::InGame))
                .after(TransformSystem::TransformPropagate),
        )
        .add_systems(
            SubstepSchedule,
            (apply_attractor_accels, apply_homing_velocity)
                .in_set(IntegrationSet::Velocity)
                .ambiguous_with_all(),
        )
        .add_systems(
            FixedPostUpdate,
            (
                (
                    detect_attracted_entities,
                    remove_attracted_initials,
                    predict_attract_trajectory,
                )
                    .chain()
                    .after(PhysicsSet::StepSimulation),
                (trigger_launch_charging, kill_out_of_bounds).after(PhysicsSet::Writeback),
            ),
        );
    }
}
