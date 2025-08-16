use bevy::ui::UiSystem;

use crate::{
    logic::{Level, LevelUnload, move_camera},
    prelude::*,
};

mod penumbra_wing_l;

#[derive(Debug, Copy, Clone, Default)]
pub struct LevelsPlugin;
impl Plugin for LevelsPlugin {
    fn build(&self, app: &mut App) {
        app.configure_sets(PostUpdate, LevelTransitionSet.before(move_camera).before(UiSystem::Content))
            .add_plugins((penumbra_wing_l::plugin,));
    }
}

#[derive(Debug, Copy, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash, SystemSet)]
pub struct LevelTransitionSet;

pub fn in_level(level_id: impl Into<String>) -> impl FnMut(Query<&Level, Without<LevelUnload>>) -> bool + Clone {
    let id = level_id.into();
    move |level: Query<&Level, Without<LevelUnload>>| {
        let Ok(level) = level.single() else {
            return false;
        };
        level.id == id
    }
}
