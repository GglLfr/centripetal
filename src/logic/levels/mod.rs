use bevy::prelude::*;

use crate::logic::{Level, LevelUnload};

mod penumbra_wing_l;

#[derive(Debug, Copy, Clone, Default)]
pub struct LevelsPlugin;
impl Plugin for LevelsPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((penumbra_wing_l::plugin,));
    }
}

pub fn in_level(
    level_id: impl Into<String>,
) -> impl FnMut(Query<&Level, Without<LevelUnload>>) -> bool + Clone {
    let id = level_id.into();
    move |level: Query<&Level, Without<LevelUnload>>| {
        let Ok(level) = level.single() else {
            return false;
        };
        level.id == id
    }
}
