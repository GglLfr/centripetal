use bevy::prelude::*;
use iyes_progress::ProgressPlugin;

use crate::GameState;

mod level;

pub use level::*;

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
        app.add_sub_state::<InGameState>()
            .add_plugins(
                ProgressPlugin::<InGameState>::new().with_state_transition(InGameState::Loading, InGameState::Resumed),
            )
            .add_event::<LoadLevelEvent>()
            .init_resource::<LevelEntities>()
            .init_resource::<LevelIntCells>()
            .add_systems(
                Update,
                (
                    handle_load_level_event.run_if(in_state(InGameState::Resumed)),
                    handle_load_level_progress
                        .after(handle_load_level_event)
                        .run_if(in_state(InGameState::Loading)),
                ),
            )
            .add_systems(OnExit(InGameState::Loading), handle_load_level_end);
    }
}
