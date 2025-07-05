use bevy::{prelude::*, render::camera::CameraUpdateSystem};
use iyes_progress::ProgressPlugin;
use leafwing_input_manager::prelude::*;

use crate::logic::{entities::EntitiesPlugin, levels::LevelsPlugin};

pub mod entities;
pub mod levels;

mod camera;
mod control;
mod level;

pub use camera::*;
pub use control::*;
pub use level::*;

#[derive(Copy, Clone, Eq, PartialEq, Debug, Hash, Default, States)]
pub enum GameState {
    #[default]
    Loading,
    Menu,
    InGame,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug, Hash, Default, SubStates)]
#[source(GameState = GameState::InGame)]
pub enum InGameState {
    Loading,
    #[default]
    Resumed,
    Paused,
}

#[derive(Debug, Copy, Clone, Default)]
pub struct LogicPlugin;
impl Plugin for LogicPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(InputManagerPlugin::<PlayerAction>::default())
            .init_state::<GameState>()
            .add_sub_state::<InGameState>()
            .add_plugins(
                ProgressPlugin::<InGameState>::new().with_state_transition(InGameState::Loading, InGameState::Resumed),
            )
            .add_event::<LoadLevelEvent>()
            .init_resource::<RegisteredLevels>()
            .init_resource::<RegisteredLevelEntities>()
            .init_resource::<RegisteredLevelIntCells>()
            .add_plugins((EntitiesPlugin, LevelsPlugin))
            .add_systems(
                Update,
                (
                    handle_load_level_event.run_if(in_state(InGameState::Resumed)),
                    handle_load_level_progress
                        .after(handle_load_level_event)
                        .run_if(in_state(InGameState::Loading)),
                ),
            )
            .add_systems(OnExit(InGameState::Loading), handle_load_level_end)
            .add_systems(Startup, startup_camera)
            .add_systems(
                PostUpdate,
                move_camera
                    .run_if(in_state(InGameState::Resumed))
                    .after(CameraUpdateSystem)
                    .before(TransformSystem::TransformPropagate),
            );
    }
}
