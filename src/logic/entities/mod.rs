use avian2d::dynamics::integrator::IntegrationSet;

use crate::{
    logic::{
        GameState, LevelApp, LevelBounds, LevelUnload,
        entities::penumbra::{
            AttractedAction, Attractor, GenericPenumbra, LaunchAction, SelenePenumbra, ThornPillar, ThornRing, apply_attractor_accels,
            apply_homing_velocity, bullet, color_selene_hurt, color_selene_parry, color_selene_slash, detect_attracted_entities,
            draw_attractor_radius, draw_selene_close, draw_selene_launch_disc, draw_selene_prediction_trajectory, draw_thorn_ring,
            predict_attract_trajectory, remove_attracted_initials, selene_cast_parry, selene_parry, trigger_launch_charging, update_launch_charging,
            update_launch_idle, update_thorn_ring_timers, warn_selene_close,
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
            self.0 = self.saturating_add_signed(delta).min(max.as_deref().copied().unwrap_or(u32::MAX));
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
    stopped: bool,
}

impl TryHurt {
    pub fn new(amount: u32) -> Self {
        Self::by(Entity::PLACEHOLDER, amount)
    }

    pub fn by(by: Entity, amount: u32) -> Self {
        Self { by, amount, stopped: false }
    }

    pub fn stop(&mut self) {
        self.stopped = true;
    }
}

impl EntityCommand for TryHurt {
    fn apply(mut self, entity: EntityWorldMut) {
        let id = entity.id();
        let world = entity.into_world_mut();

        world.trigger_targets_ref(&mut self, id);
        if !self.stopped {
            let Ok(Some(should_kill)) = world.modify_component(id, |health: &mut Health| health.hurt(self.amount)) else { return };

            world.trigger_targets(Hurt::by(self.by, self.amount), id);
            if should_kill {
                world.trigger_targets(Killed::by(self.by), id);
                if let None = world.get::<NoKillDespawn>(id) {
                    _ = world.try_despawn(id);
                }
            }
        }
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

pub fn kill_out_of_bounds(commands: ParallelCommands, level_bounds: Query<&LevelBounds, Without<LevelUnload>>, entities: Query<(Entity, &Position)>) {
    let Ok(&level_bounds) = level_bounds.single() else { return };
    entities.par_iter().for_each(|(e, &pos)| {
        if pos.x < 0. || pos.x > level_bounds.x || pos.y < 0. || pos.y > level_bounds.x {
            commands.command_scope(|mut commands| {
                commands.entity(e).queue_handled(TryHurt::new(i32::MAX as u32), warn);
            });
        }
    });
}

#[derive(Debug, Copy, Clone, Default, PhysicsLayer)]
pub enum EntityLayers {
    #[default]
    None,
    PenumbraSelene,
    PenumbraHostile,
}

impl EntityLayers {
    pub fn penumbra_selene() -> CollisionLayers {
        CollisionLayers::new(Self::PenumbraSelene, Self::PenumbraHostile)
    }

    pub fn penumbra_hostile() -> CollisionLayers {
        CollisionLayers::new(Self::PenumbraHostile, Self::PenumbraSelene)
    }
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
                color_selene_parry,
                bullet::color_spiky_spawn_effect,
                bullet::update_spiky_charge_effect,
                update_launch_idle,
                update_launch_charging,
                update_thorn_ring_timers,
                draw_selene_launch_disc,
                draw_attractor_radius,
            )
                .run_if(in_state(GameState::InGame)),
        )
        .add_systems(
            PostUpdate,
            (
                (selene_parry, selene_cast_parry)
                    .chain()
                    .in_set(RunFixedMainLoopSystem::BeforeFixedMainLoop),
                (
                    draw_selene_prediction_trajectory,
                    draw_thorn_ring,
                    (warn_selene_close, draw_selene_close).chain(),
                ),
            )
                .after(TransformSystem::TransformPropagate)
                .run_if(in_state(GameState::InGame)),
        )
        .add_systems(
            SubstepSchedule,
            (apply_attractor_accels, apply_homing_velocity)
                .in_set(IntegrationSet::Velocity)
                .ambiguous_with_all(),
        )
        .add_systems(
            FixedPostUpdate,
            (detect_attracted_entities, remove_attracted_initials)
                .chain()
                .after(PhysicsSet::StepSimulation),
        )
        .add_systems(
            PostUpdate,
            (predict_attract_trajectory, (trigger_launch_charging, kill_out_of_bounds)).in_set(RunFixedMainLoopSystem::AfterFixedMainLoop),
        );
    }
}
