use bevy::{prelude::*, ui::UiSystem};

mod scroll_text;
pub use scroll_text::*;

#[derive(Debug, Copy, Clone, Default)]
pub struct WidgetPlugin;
impl Plugin for WidgetPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            PostUpdate,
            update_scroll_text_sections.before(UiSystem::Layout),
        );
    }
}
