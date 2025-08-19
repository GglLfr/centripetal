#![allow(clippy::type_complexity)]

#[cfg(feature = "dev")]
use bevy::log::DEFAULT_FILTER;
use bevy::log::LogPlugin;
use bevy_framepace::FramepacePlugin;

use crate::{
    graphics::GraphicsPlugin,
    logic::{GameState, LoadLevel, LogicPlugin},
    prelude::*,
    ui::UiPlugin,
};

pub mod graphics;
pub mod logic;
pub mod math;
pub mod ui;

mod asset;
mod config;
mod ecs;
mod i18n;
mod save;
pub use asset::*;
pub use config::*;
pub use ecs::*;
pub use i18n::*;
pub use save::*;

pub mod prelude {
    pub use std::{
        any::{Any, TypeId},
        borrow::Cow,
        fmt, fs, io,
        marker::PhantomData,
        ops::{Deref, DerefMut, Range},
        path::{Path, PathBuf},
        str::FromStr,
        time::Duration,
    };

    pub use async_fs::File;
    pub use avian2d::{dynamics::solver::solver_body::SolverBody, prelude::*};
    pub use bevy::{
        asset::{
            AssetLoader, AssetPath, AsyncReadExt as _, AsyncWriteExt as _, LoadContext, LoadDirectError, LoadState, ParseAssetPathError,
            RecursiveDependencyLoadState, RenderAssetUsages, UntypedAssetId, VisitAssetDependencies,
            io::Reader,
            ron,
            uuid::{Uuid, uuid},
        },
        core_pipeline::core_2d::{CORE_2D_DEPTH_FORMAT, Transparent2d},
        ecs::{
            archetype::{Archetype, ArchetypeComponentId},
            bundle::{BundleEffect, DynamicBundle},
            component::{ComponentId, Components, ComponentsRegistrator, HookContext, RequiredComponents, StorageType, Tick},
            entity::{EntityHashMap, EntityHashSet},
            entity_disabling::Disabled,
            error::{ignore, warn},
            never::Never,
            query::{Access, QueryData, QueryEntityError, QueryFilter, QueryItem, QuerySingleError, ROQueryItem, ReadOnlyQueryData},
            system::{
                BoxedSystem, IntoObserverSystem, ReadOnlySystemParam, RunSystemError, RunSystemOnce as _, StaticSystemParam, SystemId, SystemMeta,
                SystemParam, SystemParamItem, SystemParamValidationError, SystemState, lifetimeless::*,
            },
            world::{DeferredWorld, OnDespawn, unsafe_world_cell::UnsafeWorldCell},
        },
        image::{ImageSampler, TextureFormatPixelInfo as _},
        math::FloatOrd,
        platform::{
            collections::{HashMap, HashSet},
            sync::{LazyLock, Mutex, MutexGuard, PoisonError, RwLock, RwLockReadGuard, RwLockWriteGuard},
        },
        prelude::*,
        ptr::{OwningPtr, Ptr, PtrMut},
        reflect::{DynamicTypePath, erased_serde},
        render::{
            MainWorld, Render, RenderApp, RenderSet,
            camera::ExtractedCamera,
            render_asset::RenderAssets,
            render_phase::{DrawFunctions, PhaseItem, RenderCommand, RenderCommandResult, RenderCommandState, TrackedRenderPass},
            render_resource::{binding_types::*, *},
            renderer::{RenderDevice, RenderQueue},
            sync_world::SyncToRenderWorld,
            texture::{CachedTexture, GpuImage, TextureCache},
            view::{ExtractedView, ViewTarget},
        },
        sprite::Anchor,
        tasks::{AsyncComputeTaskPool, ComputeTaskPool, IoTaskPool},
        ui::Val::*,
    };
    pub use bevy_asset_loader::prelude::*;
    pub use bevy_ecs_tilemap::prelude::*;
    pub use bevy_vector_shapes::{prelude::*, render::ShapePipelineType};
    pub use derive_more::{Display, Error, From, FromStr};
    pub use fastrand::Rng;
    pub use iyes_progress::prelude::*;
    pub use leafwing_input_manager::prelude::*;
    pub use serde::{Deserialize, Deserializer, Serialize, Serializer, de, ser};
    pub use smallvec::{SmallVec, smallvec};
    pub use vec_belt::VecBelt;
}

#[cfg(all(feature = "mimalloc", not(feature = "bevy_dynamic"),))]
#[global_allocator]
static ALLOC: mimalloc_redirect::MiMalloc = mimalloc_redirect::MiMalloc;

pub const PIXELS_PER_UNIT: u32 = 16;

fn main() -> AppExit {
    App::new()
        .insert_resource(ClearColor(Color::NONE))
        .add_plugins((
            DirsPlugin,
            DefaultPlugins
                .set(LogPlugin {
                    #[cfg(feature = "dev")]
                    filter: format!("{DEFAULT_FILTER},centripetal=debug"),
                    ..default()
                })
                .set(ImagePlugin::default_nearest())
                .set(WindowPlugin {
                    // Set by `ConfigPlugin`.
                    primary_window: None,
                    ..default()
                }),
            PhysicsPlugins::default().with_length_unit(PIXELS_PER_UNIT as f32),
            #[cfg(feature = "dev")]
            PhysicsDebugPlugin::default(),
            TilemapPlugin,
            FramepacePlugin,
            Shape2dPlugin::default(),
            ConfigPlugin,
            SavePlugin,
            GraphicsPlugin,
            LogicPlugin,
            UiPlugin,
        ))
        .init_asset::<I18nEntries>()
        .init_asset_loader::<I18nEntriesLoader>()
        .init_resource::<I18nContext>()
        .init_resource::<RebindObservers>()
        .add_loading_state(
            LoadingState::new(GameState::Loading)
                .load_collection::<WorldHandle>()
                .load_collection::<Sprites>()
                .load_collection::<Fonts>()
                .load_collection::<Locales>(),
        )
        .add_plugins(ProgressPlugin::<GameState>::new().with_state_transition(GameState::Loading, GameState::Menu))
        .add_systems(OnEnter(GameState::Menu), dev_init)
        .run()
}

fn dev_init(mut commands: Commands, mut state: ResMut<NextState<GameState>>) {
    debug!("[TODO remove] Dev-initialize, loading `penumbra_wing_l` now!");
    state.set(GameState::InGame);

    commands.queue(ApplySave::default().with(LoadLevel("penumbra_wing_l".into())));
}
