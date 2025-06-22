use async_channel::{Receiver, Sender};
use bevy::{
    prelude::*,
    render::{Render, RenderApp, render_asset::prepare_assets, texture::GpuImage},
    tasks::ComputeTaskPool,
};
use bevy_asset_loader::prelude::*;
use iyes_progress::prelude::*;

use crate::asset::ldtk::{Ldtk, LdtkPlugin};

pub mod ldtk;

mod sprite_sheet;
mod sprites;

pub use sprite_sheet::*;
pub use sprites::*;

#[derive(Debug, Copy, Clone, Default)]
pub struct SetupAssetPlugin;
impl Plugin for SetupAssetPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(LdtkPlugin)
            .init_state::<AssetState>()
            .add_loading_state(
                LoadingState::new(AssetState::Loading)
                    .load_collection::<WorldHandle>()
                    .load_collection::<EntityAssets>(),
            )
            .add_plugins(ProgressPlugin::<AssetState>::new().with_state_transition(AssetState::Loading, AssetState::Loaded));

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

#[derive(Clone, Eq, PartialEq, Debug, Hash, Default, States)]
pub enum AssetState {
    #[default]
    Loading,
    Loaded,
}

#[derive(Debug, Clone, Resource, AssetCollection, Deref)]
pub struct WorldHandle {
    #[asset(path = "levels/world.ldtk")]
    handle: Handle<Ldtk>,
}

#[derive(Debug, Clone, Resource, AssetCollection)]
pub struct EntityAssets {
    #[asset(path = "entities/selene/selene.json")]
    pub selene: Handle<SpriteSheet>,
}
