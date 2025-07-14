#![allow(clippy::type_complexity)]

use avian2d::prelude::*;
#[cfg(feature = "dev")]
use bevy::log::DEFAULT_FILTER;
use bevy::{log::LogPlugin, prelude::*};
use bevy_asset_loader::prelude::*;
use bevy_ecs_tilemap::prelude::*;
use bevy_framepace::FramepacePlugin;
use iyes_progress::prelude::*;

use crate::{
    graphics::{GraphicsPlugin, SpriteSection, SpriteSheet},
    logic::{GameState, Ldtk, LoadLevel, LogicPlugin},
};

pub mod graphics;
pub mod logic;
pub mod math;

mod config;
mod save;
pub use config::*;
pub use save::*;

#[cfg_attr(not(feature = "bevy_dynamic"), global_allocator)]
#[cfg_attr(
    feature = "bevy_dynamic",
    expect(unused, reason = "Bevy dynamic linking is incompatible with Mimalloc redirection")
)]
static ALLOC: mimalloc_redirect::MiMalloc = mimalloc_redirect::MiMalloc;

pub const PIXELS_PER_UNIT: u32 = 16;

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
    #[asset(path = "effects/ring_8.png")]
    pub ring_8: Handle<SpriteSection>,
    #[asset(path = "effects/ring_16.png")]
    pub ring_16: Handle<SpriteSection>,
    // Entities.
    #[asset(path = "entities/selene/selene.json")]
    pub selene: Handle<SpriteSheet>,
}

fn main() -> AppExit {
    App::new()
        .insert_resource(ClearColor(Color::NONE))
        .add_plugins((
            DirsPlugin,
            DefaultPlugins
                .set(LogPlugin {
                    #[cfg(feature = "dev")]
                    filter: format!("{DEFAULT_FILTER},centripetal=debug"),
                    ..default()
                })
                .set(ImagePlugin::default_nearest())
                .set(WindowPlugin {
                    // Set by `ConfigPlugin`.
                    primary_window: None,
                    ..default()
                }),
            PhysicsPlugins::default().with_length_unit(PIXELS_PER_UNIT as f32),
            #[cfg(feature = "dev")]
            PhysicsDebugPlugin::default(),
            TilemapPlugin,
            FramepacePlugin,
            ConfigPlugin,
            SavePlugin,
            GraphicsPlugin,
            LogicPlugin,
        ))
        .add_loading_state(
            LoadingState::new(GameState::Loading)
                .load_collection::<WorldHandle>()
                .load_collection::<Sprites>(),
        )
        .add_plugins(ProgressPlugin::<GameState>::new().with_state_transition(GameState::Loading, GameState::Menu))
        .add_systems(OnEnter(GameState::Menu), dev_init)
        .run()
}

fn dev_init(mut commands: Commands, mut state: ResMut<NextState<GameState>>) {
    debug!("[TODO remove] Dev-initialize, loading `penumbra_wing_l` now!");
    state.set(GameState::InGame);

    commands.queue(ApplySave::default().with(LoadLevel("penumbra_wing_l".into())));
}
