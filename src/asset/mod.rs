use async_channel::{Receiver, Sender};
use bevy::{
    prelude::*,
    render::{Render, RenderApp, render_asset::prepare_assets, texture::GpuImage},
    tasks::ComputeTaskPool,
};

pub mod list;

mod sprite_sheet;
mod sprites;

pub use sprite_sheet::*;
pub use sprites::*;

pub struct SetupAssetPlugin;
impl Plugin for SetupAssetPlugin {
    fn build(&self, app: &mut App) {
        let (sender, receiver) = async_channel::bounded(8);
        app.init_resource::<Sprites>()
            .init_asset::<SpriteSheet>()
            .register_asset_loader(SpriteSheetLoader(sender))
            .add_systems(PreUpdate, pack_incoming_sprites(receiver));

        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app
                .init_resource::<PendingSprites>()
                .add_systems(ExtractSchedule, extract_pending_sprites)
                .add_systems(Render, prepare_pending_sprites.after(prepare_assets::<GpuImage>));
        }
    }
}

pub fn pack_incoming_sprites(
    receiver: Receiver<(Image, Sender<Result<(Handle<Image>, TextureAtlas), SpriteError>>)>,
) -> impl System<In = (), Out = ()> {
    IntoSystem::into_system(
        move |mut sprites: ResMut<Sprites>,
              mut images: ResMut<Assets<Image>>,
              mut layouts: ResMut<Assets<TextureAtlasLayout>>| {
            ComputeTaskPool::get().scope(|scope| {
                while let Ok((image, sender)) = receiver.try_recv() {
                    let result = sprites.pack(image, &mut images, &mut layouts);
                    scope.spawn(async move {
                        _ = sender.send(result).await;
                    });
                }
            });
        },
    )
}
