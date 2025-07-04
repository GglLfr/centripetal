use avian2d::{dynamics::integrator::IntegrationSet, prelude::*};
use bevy::prelude::*;
use leafwing_input_manager::prelude::*;

#[cfg(feature = "dev")]
use crate::logic::entities::penumbra::{draw_attract_trajectory, draw_attractor_radius};
use crate::logic::{
    LevelApp, LevelLayer,
    entities::penumbra::{
        AttractedAction, Attractor, SelenePenumbra, ThornPillar, ThornRing, apply_attractor_accels,
        detect_attracted_entities, predict_attract_trajectory, update_attracted_launching,
    },
};

pub mod penumbra;

#[derive(Debug, Copy, Clone, Default)]
pub struct EntitiesPlugin;
impl Plugin for EntitiesPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(InputManagerPlugin::<AttractedAction>::default())
            .register_level_entity::<Attractor>(LevelLayer::ENTITIES, "attractor")
            .register_level_entity::<SelenePenumbra>(LevelLayer::ENTITIES, "selene_penumbra")
            .register_level_entity::<ThornPillar>(LevelLayer::ENTITIES, "thorn_pillar")
            .register_level_entity::<ThornRing>(LevelLayer::ENTITIES, "thorn_ring")
            .add_systems(Update, update_attracted_launching)
            .add_systems(
                SubstepSchedule,
                apply_attractor_accels.in_set(IntegrationSet::Velocity).ambiguous_with_all(),
            )
            .add_systems(
                PhysicsSchedule,
                (
                    predict_attract_trajectory.after(SolverSet::ApplyTranslation),
                    detect_attracted_entities.in_set(PhysicsStepSet::SpatialQuery),
                )
                    .ambiguous_with_all(),
            );

        #[cfg(feature = "dev")]
        app.add_systems(Update, (draw_attractor_radius, draw_attract_trajectory));
    }
}
