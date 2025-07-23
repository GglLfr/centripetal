use bevy::prelude::*;
use bevy_asset_loader::prelude::*;

use crate::{
    graphics::{SpriteSection, SpriteSheet},
    logic::Ldtk,
};

#[derive(Debug, Clone, Resource, AssetCollection, Deref, DerefMut)]
pub struct WorldHandle {
    #[asset(path = "levels/world.ldtk")]
    pub handle: Handle<Ldtk>,
}

#[derive(Debug, Clone, Resource, AssetCollection)]
pub struct Sprites {
    // Visual effects.
    #[asset(path = "effects/grand_attractor_spawned.json")]
    pub grand_attractor_spawned: Handle<SpriteSheet>,
    #[asset(path = "effects/ring_6.png")]
    pub ring_6: Handle<SpriteSection>,
    #[asset(path = "effects/ring_8.png")]
    pub ring_8: Handle<SpriteSection>,
    #[asset(path = "effects/ring_16.png")]
    pub ring_16: Handle<SpriteSection>,
    // Entities.
    #[asset(path = "entities/attractor/regular.json")]
    pub attractor_regular: Handle<SpriteSheet>,
    #[asset(path = "entities/attractor/slash.json")]
    pub attractor_slash: Handle<SpriteSheet>,
    #[asset(path = "entities/attractor/spawn.json")]
    pub attractor_spawn: Handle<SpriteSheet>,
    #[asset(path = "entities/generic/collectible_32.json")]
    pub collectible_32: Handle<SpriteSheet>,
    #[asset(path = "entities/selene/selene.json")]
    pub selene: Handle<SpriteSheet>,
    #[asset(path = "entities/selene/selene_penumbra.json")]
    pub selene_penumbra: Handle<SpriteSheet>,
}
