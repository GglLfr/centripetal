use std::ops::Deref;

use bevy::{
    ecs::system::SystemParam, prelude::*, render::camera::CameraUpdateSystem, time::TimeSystem,
};
use iyes_progress::{CheckProgressSet, ProgressPlugin};
use leafwing_input_manager::prelude::*;

use crate::{
    SaveApp, WorldHandle,
    logic::{effects::EffectsPlugin, entities::EntitiesPlugin, levels::LevelsPlugin},
};

pub mod effects;
pub mod entities;
pub mod levels;

mod camera;
mod control;
mod ldtk;
mod level;
mod timed;
pub use camera::*;
pub use control::*;
pub use ldtk::*;
pub use level::*;
pub use timed::*;

#[derive(SystemParam)]
pub struct LdtkWorld<'w> {
    handle: Res<'w, WorldHandle>,
    worlds: Res<'w, Assets<Ldtk>>,
}

impl LdtkWorld<'_> {
    pub fn get(&self) -> &Ldtk {
        self.worlds
            .get(self.handle.id())
            .expect("The LDtk world is unloaded")
    }
}

impl Deref for LdtkWorld<'_> {
    type Target = Ldtk;

    fn deref(&self) -> &Self::Target {
        self.get()
    }
}

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
                ProgressPlugin::<InGameState>::new()
                    .check_progress_in(PostUpdate)
                    .with_state_transition(InGameState::Loading, InGameState::Resumed),
            )
            .init_resource::<RegisteredLevels>()
            .init_resource::<RegisteredLevelEntities>()
            .init_resource::<RegisteredLevelIntCells>()
            .add_plugins((LdtkPlugin, EffectsPlugin, EntitiesPlugin, LevelsPlugin))
            .add_systems(First, update_time_stun.before(TimeSystem))
            .add_systems(
                PreUpdate,
                (handle_load_level_begin, update_timed).run_if(in_state(InGameState::Resumed)),
            )
            .add_systems(
                PostUpdate,
                handle_load_level_progress
                    .run_if(in_state(InGameState::Loading))
                    .before(CheckProgressSet),
            )
            .add_systems(OnExit(InGameState::Loading), handle_load_level_end)
            .add_systems(Startup, startup_camera)
            .add_systems(
                PostUpdate,
                move_camera
                    .run_if(in_state(InGameState::Resumed))
                    .after(CameraUpdateSystem)
                    .before(TransformSystem::TransformPropagate),
            )
            .configure_sets(
                PostUpdate,
                seldom_state::set::StateSet::Transition.before(move_camera),
            )
            .save_resource::<LoadLevel>();
    }
}
