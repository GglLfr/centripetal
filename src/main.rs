use avian2d::prelude::*;
use bevy::{prelude::*, render::camera::ScalingMode};
use bevy_ecs_tilemap::TilemapPlugin;
use bevy_framepace::FramepacePlugin;

use crate::{
    asset::{LevelUuids, SetupAssetPlugin},
    logic::{InGameState, LoadLevelEvent, LogicPlugin},
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
            TilemapPlugin,
            FramepacePlugin,
        ))
        .init_state::<GameState>()
        .add_plugins((SetupAssetPlugin, LogicPlugin))
        .add_systems(OnEnter(GameState::Menu), dev_init)
        .add_systems(Update, move_camera.run_if(in_state(InGameState::Resumed)))
        .run()
}

fn dev_init(
    mut commands: Commands,
    mut state: ResMut<NextState<GameState>>,
    mut load: EventWriter<LoadLevelEvent>,
    uuids: Res<LevelUuids>,
) {
    state.set(GameState::InGame);
    load.write(LoadLevelEvent(uuids.initial));

    commands.spawn((
        Camera2d,
        Msaa::Off,
        Projection::Orthographic(OrthographicProjection {
            scaling_mode: ScalingMode::AutoMax {
                max_width: 1920.,
                max_height: 1080.,
            },
            scale: 1. / 3.,
            ..OrthographicProjection::default_2d()
        }),
    ));
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

    trns.translation += Vec3::new(x, y, 0.) * time.delta_secs() * 5.;
}
