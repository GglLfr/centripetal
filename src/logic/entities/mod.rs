use bevy::prelude::*;

use crate::logic::{LevelApp, LevelLayer, entities::penumbra::SelenePenumbra};

pub mod penumbra;

#[derive(Debug, Copy, Clone, Default)]
pub struct EntitiesPlugin;
impl Plugin for EntitiesPlugin {
    fn build(&self, app: &mut App) {
        app.register_level_entity::<SelenePenumbra>(LevelLayer::ENTITIES, "selene_penumbra");
    }
}
