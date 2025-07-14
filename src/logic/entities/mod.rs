use avian2d::{dynamics::integrator::IntegrationSet, prelude::*};
use bevy::prelude::*;
use leafwing_input_manager::prelude::*;

#[cfg(feature = "dev")]
use crate::logic::entities::penumbra::{draw_attract_trajectory, draw_attractor_radius};
use crate::logic::{
    LevelApp,
    entities::penumbra::{
        AttractedAction, Attractor, GenericPenumbra, SelenePenumbra, ThornPillar, ThornRing, apply_attractor_accels,
        detect_attracted_entities, predict_attract_trajectory, remove_attracted_initials, update_attracted_launching,
    },
};

pub mod penumbra;

#[derive(Debug, Copy, Clone, Component, Deref)]
#[component(immutable)]
pub struct Health(pub u32);
impl Health {
    pub fn hurt(self, amount: u32) -> Self {
        self.change(-(amount as i32), None)
    }

    pub fn change(self, delta: i32, max: Option<MaxHealth>) -> Self {
        Self(
            self.saturating_add_signed(delta)
                .min(max.as_deref().copied().unwrap_or(u32::MAX)),
        )
    }
}

#[derive(Debug, Copy, Clone, Component, Deref)]
#[component(immutable)]
pub struct MaxHealth(pub u32);

#[derive(Debug, Copy, Clone, Event)]
pub struct Hurt {
    pub by: Entity,
    pub amount: u32,
}

impl Hurt {
    pub fn new(by: Entity, amount: u32) -> Self {
        Self { by, amount }
    }
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
                apply_attractor_accels.in_set(IntegrationSet::Velocity).ambiguous_with_all(),
            )
            .add_systems(
                PhysicsSchedule,
                (
                    predict_attract_trajectory.after(SolverSet::Finalize),
                    (detect_attracted_entities, remove_attracted_initials)
                        .chain()
                        .in_set(PhysicsStepSet::SpatialQuery)
                        .after(update_spatial_query_pipeline),
                )
                    .ambiguous_with_all(),
            );

        #[cfg(feature = "dev")]
        app.add_systems(Update, (draw_attractor_radius, draw_attract_trajectory));
    }
}
