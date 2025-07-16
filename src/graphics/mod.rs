use bevy::{
    prelude::*,
    render::{Render, RenderApp, render_asset::prepare_assets, texture::GpuImage},
};
use seldom_state::set::StateSet;

mod animation;
mod sprite_alloc;
mod sprite_drawer;
mod sprite_sheet;

pub use animation::*;
pub use sprite_alloc::*;
pub use sprite_drawer::*;
pub use sprite_sheet::*;

#[derive(Debug, Copy, Clone, Component, Deref, DerefMut)]
pub struct EntityColor(pub Color);
impl Default for EntityColor {
    fn default() -> Self {
        Self(Color::WHITE)
    }
}

#[derive(Debug, Copy, Clone, Default)]
pub struct GraphicsPlugin;
impl Plugin for GraphicsPlugin {
    fn build(&self, app: &mut App) {
        app.init_asset::<SpriteSection>().init_asset::<SpriteSheet>().add_systems(
            PostUpdate,
            (
                update_animations.before(StateSet::Transition),
                draw_animations,
                flush_drawer_to_children,
            )
                .chain()
                .before(TransformSystem::TransformPropagate),
        );

        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app
                .init_resource::<PendingSprites>()
                .add_systems(ExtractSchedule, extract_pending_sprites)
                .add_systems(Render, prepare_pending_sprites.after(prepare_assets::<GpuImage>));
        }
    }

    fn finish(&self, app: &mut App) {
        let (sender, receiver) = async_channel::bounded(8);
        app.init_resource::<SpriteAllocator>()
            .register_asset_loader(SpriteSheetLoader(sender.clone()))
            .register_asset_loader(SpriteSectionLoader(sender))
            .add_systems(PreUpdate, pack_incoming_sprites(receiver));
    }
}
