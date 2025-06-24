use bevy::prelude::*;

mod material;

pub use material::*;

#[derive(Debug, Copy, Clone, Default)]
pub struct GfxPlugin;
impl Plugin for GfxPlugin {
    fn build(&self, app: &mut App) {
        //
    }
}
