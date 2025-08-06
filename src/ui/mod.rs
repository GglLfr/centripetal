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
        app.add_plugins(WidgetPlugin)
            .add_systems(
                PostUpdate,
                (
                    update_worldspace_ui
                        .after(UiSystem::Prepare)
                        .before(UiSystem::Layout),
                    fade_interpolate,
                ),
            )
            .add_observer(on_fade_insert)
            .add_observer(on_fade_done);
    }
}

#[derive(Debug, Copy, Clone, Component)]
#[component(storage = "SparseSet")]
struct PreviousDisplay(Display);

pub fn ui_hide(mut e: EntityWorldMut) {
    let prev = e
        .get_mut::<Node>()
        .map(|mut node| std::mem::replace(&mut node.display, Display::None));

    if let Some(prev) = prev {
        e.insert(PreviousDisplay(prev));
    }
}

pub fn ui_show(mut e: EntityWorldMut) {
    if let Some(prev) = e.take::<PreviousDisplay>()
        && let Some(mut node) = e.get_mut::<Node>()
    {
        node.display = prev.0;
    }
}
