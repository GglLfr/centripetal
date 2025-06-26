use avian2d::prelude::*;
use bevy::prelude::*;
use bevy_ecs_tilemap::TilemapPlugin;
use bevy_framepace::FramepacePlugin;

use crate::{
    asset::SetupAssetPlugin,
    gfx::GfxPlugin,
    logic::{GameState, InGameState, LoadLevelEvent, LogicPlugin},
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
            PhysicsPlugins::default().with_length_unit(16.),
            #[cfg(feature = "dev")]
            PhysicsDebugPlugin::default(),
            TilemapPlugin,
            FramepacePlugin,
        ))
        .add_plugins((SetupAssetPlugin, LogicPlugin, GfxPlugin))
        .add_systems(OnEnter(GameState::Menu), dev_init)
        .add_systems(Update, move_camera.run_if(in_state(InGameState::Resumed)))
        .run()
}

fn dev_init(mut state: ResMut<NextState<GameState>>, mut load: EventWriter<LoadLevelEvent>) {
    state.set(GameState::InGame);
    load.write(LoadLevelEvent("penumbra_wing_l".into()));
}

fn move_camera(mut trns: Single<&mut Transform, With<Camera2d>>, input: Res<ButtonInput<KeyCode>>, time: Res<Time>) {
    let [mut x, mut y] = [0., 0.];
    if input.pressed(KeyCode::KeyW) {
        y += 1.
    }
    if input.pressed(KeyCode::KeyS) {
        y -= 1.
    }
    if input.pressed(KeyCode::KeyA) {
        x -= 1.
    }
    if input.pressed(KeyCode::KeyD) {
        x += 1.
    }

    trns.translation += Vec3::new(x, y, 0.) * time.delta_secs() * 3.5;
}
