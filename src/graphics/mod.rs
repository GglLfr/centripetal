use bevy::{
    prelude::*,
    render::{Render, RenderApp, render_asset::prepare_assets, texture::GpuImage},
};

mod animation;
mod sprite_drawer;
mod sprite_sheet;
mod sprites;

pub use animation::*;
pub use sprite_drawer::*;
pub use sprite_sheet::*;
pub use sprites::*;

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
        app.add_systems(
            PostUpdate,
            (update_animations, draw_animations, flush_drawer_to_children)
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
        app.init_resource::<Sprites>()
            .init_asset::<SpriteSheet>()
            .register_asset_loader(SpriteSheetLoader(sender))
            .add_systems(PreUpdate, pack_incoming_sprites(receiver));
    }
}
