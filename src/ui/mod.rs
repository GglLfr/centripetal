use bevy::{prelude::*, ui::UiSystem};

pub mod widgets;

mod fade;
mod worldspace;
pub use fade::*;
pub use worldspace::*;

use crate::ui::widgets::WidgetPlugin;

#[derive(Debug, Copy, Clone, Default)]
pub struct UiPlugin;
impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(WidgetPlugin).add_systems(
            PostUpdate,
            update_worldspace_ui
                .after(UiSystem::Prepare)
                .before(UiSystem::Layout),
        );
    }
}
