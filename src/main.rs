mod asset;
mod progress;
pub use asset::*;
pub use progress::*;

#[cfg(feature = "dev")]
pub mod editor;
pub mod entities;
pub mod math;
pub mod render;
pub mod saves;
pub mod util;
pub mod world;

pub mod prelude {
    pub use std::{
        any::{TypeId, type_name},
        collections::BTreeMap,
        f32::consts::{PI, TAU},
        fmt::{self, Debug},
        hash::{Hash, Hasher},
        marker::PhantomData,
        mem::{self, MaybeUninit, offset_of},
        ops::Range,
        time::Duration,
    };

    pub use avian2d::prelude::*;
    #[cfg(feature = "dev")]
    pub use bevy::ui_widgets;
    pub use bevy::{
        asset::{
            AssetLoader, LoadContext, LoadState, RecursiveDependencyLoadState, RenderAssetUsages, UntypedAssetId, io::Reader, load_embedded_asset,
        },
        camera::{
            ImageRenderTarget, RenderTarget,
            primitives::Aabb,
            visibility::{RenderLayers, VisibilityClass, add_visibility_class},
        },
        core_pipeline::{
            core_2d::{CORE_2D_DEPTH_FORMAT, Transparent2d},
            tonemapping::{DebandDither, Tonemapping, TonemappingLuts, get_lut_bind_group_layout_entries, get_lut_bindings},
        },
        ecs::{
            component::Tick,
            entity::{EntityHash, EntityHashMap, MapEntities},
            hierarchy::validate_parent_has_component,
            intern::Interned,
            lifecycle::HookContext,
            query::{FilteredAccessSet, ROQueryItem},
            reflect::ReflectMapEntities,
            relationship::RelationshipHookMode,
            schedule::ScheduleLabel,
            system::{
                RunSystemError, RunSystemOnce, SystemMeta, SystemParam, SystemParamItem, SystemParamValidationError, SystemState,
                lifetimeless::{Read, SRes},
            },
            world::{DeferredWorld, unsafe_world_cell::UnsafeWorldCell},
        },
        image::{CompressedImageFormats, ImageLoader, ImageLoaderSettings},
        math::{Affine2, FloatOrd},
        mesh::{Indices, MeshVertexAttribute, VertexAttributeValues, VertexBufferLayout},
        platform::{
            collections::{HashMap, HashSet},
            sync::{
                Arc,
                atomic::{AtomicU64, Ordering},
            },
        },
        prelude::*,
        reflect::{FromType, Reflectable, TypeRegistry, erased_serde},
        render::{
            Extract, MainWorld, Render, RenderApp, RenderStartup, RenderSystems,
            render_asset::RenderAssets,
            render_phase::{
                AddRenderCommand as _, DrawFunctions, PhaseItem, PhaseItemExtraIndex, RenderCommand, RenderCommandResult, SetItemPipeline,
                TrackedRenderPass, ViewSortedRenderPhases,
            },
            render_resource::{binding_types::*, *},
            renderer::{RenderDevice, RenderQueue},
            storage::ShaderStorageBuffer,
            sync_component::SyncComponentPlugin,
            sync_world::{MainEntity, RenderEntity},
            texture::{FallbackImage, GpuImage},
            view::{ExtractedView, Hdr, RenderVisibleEntities, RetainedViewEntity, ViewTarget, ViewUniform, ViewUniformOffset, ViewUniforms},
        },
        shader::ShaderDefVal,
        sprite::Anchor,
        sprite_render::SpritePipelineKey,
        state::state::FreelyMutableState,
        tasks::{AsyncComputeTaskPool, ComputeTaskPool, IoTaskPool},
        window::PrimaryWindow,
    };
    pub use bevy_framepace::FramepacePlugin;
    pub use bitflags::{bitflags, bitflags_match};
    pub use bytemuck::{Pod, Zeroable, must_cast_slice as cast_slice, must_cast_slice_mut as cast_slice_mut};
    pub use serde::{
        Deserialize, Deserializer, Serialize, Serializer,
        de::{self, DeserializeSeed},
        ser::{self, SerializeMap, SerializeSeq, SerializeStruct, SerializeTuple},
    };
    pub use slab::Slab;
    pub use smallvec::{SmallVec, smallvec};
    pub use vec_belt::{Transfer, VecBelt};
}

use prelude::*;

#[cfg(not(target_family = "wasm"))]
#[global_allocator]
static ALLOC: mimalloc_redirect::MiMalloc = mimalloc_redirect::MiMalloc;

#[cfg(not(target_family = "wasm"))]
fn print_mimalloc_version(_: &mut App) {
    info!("Using MiMalloc {}", mimalloc_redirect::MiMalloc::get_version());
}

#[inline(always)]
pub fn patched<T>(func: impl FnMut() -> T) -> T {
    #[cfg(feature = "dev")]
    return bevy::app::hotpatch::call(func);

    #[cfg(not(feature = "dev"))]
    func()
}

pub const PIXELS_PER_METER: f32 = 16.;

#[derive(Reflect, States, Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
#[reflect(State, Debug, Default, FromWorld, Clone, PartialEq, Hash)]
pub enum GameState {
    #[default]
    AssetLoading,
    Menu,
    InGame {
        paused: bool,
    },
    #[cfg(feature = "dev")]
    Editor,
}

pub fn main() -> AppExit {
    App::new()
        .add_plugins((
            DefaultPlugins.set(ImagePlugin::default_nearest()),
            #[cfg(feature = "dev")]
            ui_widgets::UiWidgetsPlugins,
            PhysicsPlugins::default().with_length_unit(PIXELS_PER_METER),
            FramepacePlugin,
            #[cfg(not(target_family = "wasm"))]
            print_mimalloc_version,
        ))
        .init_state::<GameState>()
        .add_plugins((
            ProgressPlugin::default().trans(GameState::AssetLoading, GameState::Menu),
            asset::plugin,
            #[cfg(feature = "dev")]
            editor::plugin,
            entities::plugin,
            render::plugin,
            saves::plugin,
            world::plugin,
        ))
        .add_systems(
            OnExit(GameState::AssetLoading),
            |#[cfg_attr(not(feature = "dev"), expect(unused))] mut commands: Commands| {
                #[cfg(feature = "dev")]
                commands.run_system_cached(editor::start);
            },
        )
        .run()
}
