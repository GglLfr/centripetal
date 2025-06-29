use avian2d::{dynamics::integrator::IntegrationSet, prelude::*};
use bevy::prelude::*;

#[cfg(feature = "dev")]
use crate::logic::entities::penumbra::draw_attractor_radius;
use crate::logic::{
    LevelApp, LevelLayer,
    entities::penumbra::{Attractor, SelenePenumbra, apply_attractor_accels, detect_attracted_entities},
};

pub mod penumbra;

#[derive(Debug, Copy, Clone, Default)]
pub struct EntitiesPlugin;
impl Plugin for EntitiesPlugin {
    fn build(&self, app: &mut App) {
        app.register_level_entity::<SelenePenumbra>(LevelLayer::ENTITIES, "selene_penumbra")
            .register_level_entity::<Attractor>(LevelLayer::ENTITIES, "attractor")
            .add_systems(
                SubstepSchedule,
                apply_attractor_accels.in_set(IntegrationSet::Velocity).ambiguous_with_all(),
            )
            .add_systems(
                PhysicsSchedule,
                detect_attracted_entities
                    .in_set(PhysicsStepSet::SpatialQuery)
                    .ambiguous_with_all(),
            );

        #[cfg(feature = "dev")]
        app.add_systems(Update, draw_attractor_radius);
    }
}
