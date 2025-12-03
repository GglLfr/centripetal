mod asset;
mod control;
mod progress;

pub use asset::*;
pub use control::*;
pub use progress::*;

pub mod entities;
pub mod math;
pub mod render;
pub mod saves;
pub mod util;
pub mod world;

pub mod prelude {
    pub use std::{
        any::{Any, TypeId, type_name},
        borrow::Cow,
        collections::BTreeMap,
        f32::consts::{PI, TAU},
        fmt::{self, Debug},
        fs,
        hash::{Hash, Hasher},
        io,
        marker::PhantomData,
        mem::{self, MaybeUninit, offset_of},
        ops::{Deref, DerefMut, Mul, Range, RangeInclusive},
        path::{Path, PathBuf},
        ptr::NonNull,
        str::FromStr,
        time::Duration,
    };

    pub use atomicow::CowArc;
    pub use avian2d::prelude::*;
    pub use bevy::{
        asset::{
            AsAssetId, AssetIndex, AssetLoader, AssetPath, AsyncReadExt as _, LoadContext, LoadState, RecursiveDependencyLoadState, ReflectAsset,
            RenderAssetUsages, UntypedAssetId, VisitAssetDependencies,
            io::{
                AssetSourceBuilder, AssetSourceId, AssetWriterError, Reader,
                file::{FileAssetReader, FileAssetWriter},
            },
            load_embedded_asset, ron,
            uuid::{Uuid, uuid},
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
            bundle::InsertMode,
            change_detection::MaybeLocation,
            component::{ComponentId, Tick},
            entity::{Entities, EntityHash, EntityHashMap, MapEntities},
            entity_disabling::Disabled,
            error::{CommandWithEntity, ErrorContext, HandleError, error, warn},
            hierarchy::validate_parent_has_component,
            intern::Interned,
            lifecycle::HookContext,
            query::{FilteredAccessSet, ROQueryItem},
            reflect::ReflectMapEntities,
            relationship::RelationshipHookMode,
            schedule::ScheduleLabel,
            system::{
                BoxedReadOnlySystem, FilteredResourcesParamBuilder, QueryParamBuilder, RunSystemError, RunSystemOnce, SystemMeta, SystemParam,
                SystemParamItem, SystemParamValidationError, SystemState, entity_command,
                lifetimeless::{Read, SRes},
            },
            world::{CommandQueue, DeferredWorld, FilteredEntityRef, unsafe_world_cell::UnsafeWorldCell},
        },
        image::{ImageLoader, ImageLoaderSettings},
        math::{Affine2, FloatOrd},
        mesh::{Indices, MeshVertexAttribute, VertexAttributeValues, VertexBufferLayout},
        platform::{
            collections::{Equivalent, HashMap, HashSet},
            sync::{
                Arc,
                atomic::{AtomicU64, Ordering},
            },
        },
        prelude::*,
        reflect::{FromType, Reflectable, TypeRegistry, TypeRegistryArc, erased_serde},
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
        sprite_render::{AlphaMode2d, SpritePipelineKey},
        state::state::FreelyMutableState,
        tasks::{AsyncComputeTaskPool, ComputeTaskPool, ConditionalSendFuture, IoTaskPool, Task, futures::check_ready, futures_lite},
        utils::Parallel,
        window::PrimaryWindow,
    };
    pub use bevy_enhanced_input::{
        action::events::Cancel,
        condition::{press::Press, release::Release},
        prelude::*,
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
    LevelLoading,
    InGame {
        paused: bool,
    },
}

pub fn main() -> AppExit {
    std::panic::set_hook(Box::new(|info| {
        let backtrace = format!(
            "{}\n{}",
            info.payload_as_str().unwrap_or("Unknown error payload message"),
            std::backtrace::Backtrace::force_capture()
        );

        let log_name = match time::UtcOffset::current_local_offset()
            .ok()
            .and_then(|offset| time::UtcDateTime::now().checked_to_offset(offset))
            .and_then(|time| {
                time.format(&time::macros::format_description!("[year]-[month]-[day]_[hour]-[minute]-[second]"))
                    .ok()
            }) {
            Some(time) => format!("centripetal_crashlog_{time}.log"),
            None => "centripetal_crash.log".into(),
        };

        let log_file = std::env::current_dir()
            .inspect_err(|e| warn!("Couldn't get executable path: {e}"))
            .ok()
            .unwrap_or_default()
            .join(log_name);

        error!("{backtrace}");
        tfd::MessageBox::new(
            "Crash!",
            &format!(
                "An unrecoverable error has occured in Centripetal. A crash log has been written at {} which contains the error message and backtrace below.\nPlease report this to https://github.com/GglLfr/centripetal\n\n{backtrace}",
                log_file.display(),
            ),
        ).with_icon(tfd::MessageBoxIcon::Error).run_modal();

        #[cfg(not(feature = "dev"))]
        if let Err(e) = fs::File::create(log_file).and_then(|mut file| {
            use std::io::Write;

            file.write_all(backtrace.as_bytes())?;
            file.sync_all()
        }) {
            tfd::MessageBox::new(
                "Worse than crash!",
                &format!("Couldn't write crash log file: {e}\n\nSure hope you can copy the crashlog text in some other way..."),
            )
            .with_icon(tfd::MessageBoxIcon::Error)
            .run_modal();
        }
    }));

    App::new()
        .add_plugins((
            #[cfg(not(target_family = "wasm"))]
            print_mimalloc_version,
            DefaultPlugins
                .set(ImagePlugin::default_nearest())
                .add_before::<AssetPlugin>(asset::register_user_sources),
            PhysicsPlugins::default().with_length_unit(PIXELS_PER_METER),
            #[cfg(feature = "dev")]
            PhysicsDebugPlugin::default(),
            EnhancedInputPlugin,
            FramepacePlugin,
        ))
        .init_state::<GameState>()
        .add_plugins((
            ProgressPlugin::default()
                .trans(GameState::AssetLoading, GameState::Menu)
                .trans(GameState::LevelLoading, GameState::InGame { paused: false }),
            asset::plugin,
            entities::plugin,
            math::plugin,
            render::plugin,
            saves::plugin,
            util::plugin,
            world::plugin,
        ))
        .add_systems(OnExit(GameState::AssetLoading), |mut load_level: ResMut<world::LoadLevel>| {
            load_level.load("eastern_beacon");
        })
        .run()
}
