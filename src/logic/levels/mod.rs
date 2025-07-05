use bevy::prelude::*;

use crate::logic::LevelApp;

mod penumbra_wing_l;

pub use penumbra_wing_l::*;

#[derive(Debug, Copy, Clone, Default)]
pub struct LevelsPlugin;
impl Plugin for LevelsPlugin {
    fn build(&self, app: &mut App) {
        app.register_level::<PenumbraWingL>("penumbra_wing_l");
    }
}
