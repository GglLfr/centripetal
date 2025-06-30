use avian2d::prelude::*;
#[cfg(feature = "dev")]
use bevy::log::DEFAULT_FILTER;
use bevy::{log::LogPlugin, prelude::*};
use bevy_ecs_tilemap::TilemapPlugin;
use bevy_framepace::FramepacePlugin;

use crate::{
    asset::SetupAssetPlugin,
    gfx::GfxPlugin,
    logic::{GameState, LoadLevelEvent, LogicPlugin},
};

pub mod asset;
pub mod gfx;
pub mod logic;

mod config;
pub use config::*;

#[cfg_attr(not(feature = "bevy_dynamic"), global_allocator)]
#[cfg_attr(
    feature = "bevy_dynamic",
    expect(unused, reason = "Bevy dynamic linking is incompatible with Mimalloc redirection")
)]
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
            TilemapPlugin,
            FramepacePlugin,
            SetupAssetPlugin,
            LogicPlugin,
            GfxPlugin,
            ConfigPlugin,
        ))
        .add_systems(OnEnter(GameState::Menu), dev_init)
        .run()
}

fn dev_init(mut state: ResMut<NextState<GameState>>, mut load: EventWriter<LoadLevelEvent>) {
    debug!("[TODO remove] Dev-initialize, loading `penumbra_wing_l` now!");
    state.set(GameState::InGame);
    load.write(LoadLevelEvent("penumbra_wing_l".into()));
}
