use crate::prelude::*;

mod ring;
pub use ring::*;

#[derive(Debug, Copy, Clone, Default)]
pub struct EffectsPlugin;
impl Plugin for EffectsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, update_ring);
    }
}
