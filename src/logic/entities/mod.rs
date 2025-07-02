use avian2d::{dynamics::integrator::IntegrationSet, prelude::*};
use bevy::prelude::*;
use leafwing_input_manager::{plugin::InputManagerSystem, prelude::*};

#[cfg(feature = "dev")]
use crate::logic::entities::penumbra::{draw_attract_trajectory, draw_attractor_radius};
use crate::logic::{
    LevelApp, LevelLayer,
    entities::penumbra::{
        AttractedAction, Attractor, OnLaunch, SelenePenumbra, ThornPillar, apply_attractor_accels,
        copy_player_to_hover_state, detect_attracted_entities, predict_attract_trajectory, update_attracted_launching,
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
            .add_event::<OnLaunch>()
            .add_systems(PreUpdate, copy_player_to_hover_state.after(InputManagerSystem::ManualControl))
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
