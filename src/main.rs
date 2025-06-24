use avian2d::prelude::*;
use bevy::prelude::*;
use bevy_framepace::FramepacePlugin;

use crate::{
    asset::{LevelUuids, SetupAssetPlugin},
    logic::{LoadLevelEvent, LogicPlugin},
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

#[derive(Copy, Clone, Eq, PartialEq, Debug, Hash, Default, States)]
pub enum GameState {
    #[default]
    Loading,
    Menu,
    InGame,
}

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
            ConfigPlugin,
            PhysicsPlugins::default(),
            #[cfg(feature = "dev")]
            PhysicsDebugPlugin::default(),
            FramepacePlugin,
        ))
        .init_state::<GameState>()
        .add_plugins((SetupAssetPlugin, LogicPlugin))
        .add_systems(OnEnter(GameState::Menu), dev_init)
        .run()
}

fn dev_init(mut commands: Commands, mut state: ResMut<NextState<GameState>>, mut load: EventWriter<LoadLevelEvent>, uuids: Res<LevelUuids>) {
    state.set(GameState::InGame);
    load.write(LoadLevelEvent(uuids.initial));

    commands.spawn(Camera2d);
}
