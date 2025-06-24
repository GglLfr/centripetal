use std::ops::Deref;

use async_channel::{Receiver, Sender};
use bevy::{
    asset::uuid::Uuid,
    ecs::system::{SystemParam, SystemState},
    prelude::*,
    render::{Render, RenderApp, render_asset::prepare_assets, texture::GpuImage},
    tasks::ComputeTaskPool,
};
use bevy_asset_loader::prelude::*;
use iyes_progress::ProgressPlugin;

use crate::{
    GameState,
    asset::ldtk::{Ldtk, LdtkPlugin},
};

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
            .add_loading_state(
                LoadingState::new(GameState::Loading)
                    .load_collection::<WorldHandle>()
                    .load_collection::<EntityAssets>()
                    .finally_init_resource::<LevelUuids>(),
            )
            .add_plugins(ProgressPlugin::<GameState>::new().with_state_transition(GameState::Loading, GameState::Menu));

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

#[derive(Debug, Clone, Resource, AssetCollection, Deref, DerefMut)]
pub struct WorldHandle {
    #[asset(path = "levels/world.ldtk")]
    pub handle: Handle<Ldtk>,
}

#[derive(SystemParam)]
pub struct LdtkWorld<'w> {
    handle: Res<'w, WorldHandle>,
    worlds: Res<'w, Assets<Ldtk>>,
}

impl LdtkWorld<'_> {
    pub fn get(&self) -> &Ldtk {
        self.worlds.get(self.handle.id()).expect("The LDtk world is unloaded")
    }
}

impl Deref for LdtkWorld<'_> {
    type Target = Ldtk;

    fn deref(&self) -> &Self::Target {
        self.get()
    }
}

#[derive(Debug, Copy, Clone, Resource)]
pub struct LevelUuids {
    /// Codename `sanctuary_chapel_nw`.
    pub initial: Uuid,
}

impl FromWorld for LevelUuids {
    fn from_world(world: &mut World) -> Self {
        let world = &*SystemState::<LdtkWorld>::new(world).get(world);
        let load = |name| {
            *world
                .level_identifiers
                .get(name)
                .unwrap_or_else(|| panic!("Level with codename {name} not found"))
        };

        Self {
            initial: load("sanctuary_chapel_nw"),
        }
    }
}

#[derive(Debug, Clone, Resource, AssetCollection)]
pub struct EntityAssets {
    #[asset(path = "entities/selene/selene.json")]
    pub selene: Handle<SpriteSheet>,
}
