use bevy::prelude::*;

pub mod penumbra_wing_l;

#[derive(Debug, Copy, Clone, Default)]
pub struct LevelsPlugin;
impl Plugin for LevelsPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((penumbra_wing_l::plugin,));
    }
}
