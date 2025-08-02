use bevy::{prelude::*, ui::UiSystem};

pub mod widgets;

mod worldspace;
pub use worldspace::*;

#[derive(Debug, Copy, Clone, Default)]
pub struct UiPlugin;
impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            PostUpdate,
            update_worldspace_ui
                .after(UiSystem::Prepare)
                .before(UiSystem::Layout),
        );
    }
}
