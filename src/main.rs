use avian2d::prelude::*;
use bevy::prelude::*;
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

#[cfg_attr(not(feature = "dev"), global_allocator)]
#[cfg_attr(
    feature = "dev",
    expect(unused, reason = "Bevy dynamic linking is incompatible with Mimalloc redirection")
)]
static ALLOC: mimalloc_redirect::MiMalloc = mimalloc_redirect::MiMalloc;

pub const PIXELS_PER_UNIT: f32 = 32.;

fn main() -> AppExit {
    App::new()
        .insert_resource(ClearColor(Color::NONE))
        .add_plugins((
            DirsPlugin,
            DefaultPlugins.set(ImagePlugin::default_nearest()).set(WindowPlugin {
                // Set by `ConfigPlugin`.
                primary_window: None,
                ..default()
            }),
            PhysicsPlugins::default().with_length_unit(PIXELS_PER_UNIT),
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
    state.set(GameState::InGame);
    load.write(LoadLevelEvent("penumbra_wing_l".into()));
}
