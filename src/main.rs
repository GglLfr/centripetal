#![allow(clippy::type_complexity)]

use avian2d::prelude::*;
#[cfg(feature = "dev")]
use bevy::log::DEFAULT_FILTER;
use bevy::{log::LogPlugin, prelude::*};
use bevy_asset_loader::prelude::*;
use bevy_ecs_tilemap::prelude::*;
use bevy_framepace::FramepacePlugin;
use bevy_vector_shapes::prelude::*;
use iyes_progress::prelude::*;
use seldom_state::prelude::*;

use crate::{
    graphics::GraphicsPlugin,
    logic::{GameState, LoadLevel, LogicPlugin},
};

pub mod graphics;
pub mod logic;
pub mod math;

mod asset;
mod config;
mod ecs;
mod save;
pub use asset::*;
pub use config::*;
pub use ecs::*;
pub use save::*;

#[cfg(not(feature = "bevy_dynamic"))]
#[global_allocator]
static ALLOC: mimalloc_redirect::MiMalloc = mimalloc_redirect::MiMalloc;

pub const PIXELS_PER_UNIT: u32 = 16;

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
            StateMachinePlugin::default(),
            TilemapPlugin,
            FramepacePlugin,
            Shape2dPlugin::default(),
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
        .add_plugins(
            ProgressPlugin::<GameState>::new()
                .with_state_transition(GameState::Loading, GameState::Menu),
        )
        .add_systems(OnEnter(GameState::Menu), dev_init)
        .run()
}

fn dev_init(mut commands: Commands, mut state: ResMut<NextState<GameState>>) {
    debug!("[TODO remove] Dev-initialize, loading `penumbra_wing_l` now!");
    state.set(GameState::InGame);

    commands.queue(ApplySave::default().with(LoadLevel("penumbra_wing_l".into())));
}
