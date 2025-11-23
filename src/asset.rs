pub use centripetal_macros::MapAssetIds;

use crate::{GameState, ProgressSystems, prelude::*, progress::ProgressFor, render::atlas::AtlasRegion};

macro_rules! define_collection {
    ($(#[$attr:meta])* $vis:vis $name:ident { $($asset_name:ident: $asset_type:path = $asset_path:expr),* }) => {
        #[derive(Resource, Debug)]
        $(#[$attr])*
        $vis struct $name {
            $vis $($asset_name: Handle<$asset_type>,)*
        }

        impl $name {
            $vis fn init(mut commands: Commands, server: Res<AssetServer>, mut tracker: ResMut<AssetLoadingTracker>) {
                $(
                    let $asset_name = server.load($asset_path);
                    tracker.loading.push($asset_name.id().untyped());
                )*

                commands.insert_resource(Self {
                    $($asset_name,)*
                });
            }

            $vis fn iter_ids(&self) -> impl Iterator<Item = UntypedAssetId> {
                [
                    $(self.$asset_name.id().untyped(),)*
                ].into_iter()
            }
        }
    };
}

define_collection! {
    pub CharacterTextures {
        selene: AtlasRegion = "entities/characters/selene/selene.png"
    }
}

define_collection! {
    pub TileTextures {
        tmp: AtlasRegion = "entities/characters/selene/selene.png"
    }
}

#[derive(Resource, Debug, Default)]
pub struct AssetLoadingTracker {
    loading: Vec<UntypedAssetId>,
    loaded: u32,
}

fn track_asset_loading(progress: ProgressFor<GameState>, mut tracker: ResMut<AssetLoadingTracker>, server: Res<AssetServer>) -> Result {
    let mut current = 0;
    let mut total = tracker.loaded * 2;

    let mut i = 0;
    while i < tracker.loading.len() {
        let mut done = false;
        let id = tracker.loading[i];

        current += match (server.load_state(id), server.recursive_dependency_load_state(id)) {
            (LoadState::Failed(e), ..) | (.., RecursiveDependencyLoadState::Failed(e)) => Err(e)?,
            (LoadState::NotLoaded, ..) | (.., RecursiveDependencyLoadState::NotLoaded) => Err("Some asset handles got dropped")?,
            (LoadState::Loaded, RecursiveDependencyLoadState::Loaded) => {
                done = true;
                2
            }
            (LoadState::Loaded, ..) | (.., RecursiveDependencyLoadState::Loaded) => 1,
            (LoadState::Loading, RecursiveDependencyLoadState::Loading) => 0,
        };
        total += 2;

        if done {
            tracker.loading.swap_remove(i);
        } else {
            i += 1
        }
    }

    progress.update([current, total]);
    Ok(())
}

pub trait MapAssetIds: 'static {
    fn map_asset_ids(&mut self, mapper: &mut dyn AssetIdMapper);
}

pub trait AssetIdMapper {
    fn map(&mut self, id: UntypedAssetId) -> UntypedAssetId;
}

#[derive(Clone, Copy)]
pub struct ReflectMapAssetIds {
    map_asset_ids: fn(&mut dyn PartialReflect, &mut dyn AssetIdMapper),
}

impl ReflectMapAssetIds {
    pub fn map_asset_ids(&self, target: &mut dyn PartialReflect, mapper: &mut dyn AssetIdMapper) {
        (self.map_asset_ids)(target, mapper)
    }
}

impl<T: MapAssetIds> FromType<T> for ReflectMapAssetIds {
    fn from_type() -> Self {
        Self {
            map_asset_ids: |target, mapper| {
                let target = target.try_downcast_mut::<T>().expect("Wrong type provided");
                target.map_asset_ids(mapper);
            },
        }
    }
}

impl<T: Asset> MapAssetIds for AssetId<T> {
    fn map_asset_ids(&mut self, mapper: &mut dyn AssetIdMapper) {
        *self = mapper.map(self.untyped()).typed_debug_checked();
    }
}

pub(super) fn register_user_sources(app: &mut App) {
    let (pref_dir, data_dir) = directories::ProjectDirs::from("com.github", "GglLfr", "Centripetal")
        .map(|dirs| (dirs.preference_dir().to_path_buf(), dirs.data_dir().to_path_buf()))
        .unwrap_or_else(|| {
            error!("Couldn't get application directories; creating a local folder instead!");
            (PathBuf::from("Centripetal Data/preference"), PathBuf::from("Centripetal Data/data"))
        });

    if let (Err(e), ..) | (.., Err(e)) = (fs::create_dir_all(&pref_dir), fs::create_dir_all(&data_dir)) {
        panic!("Couldn't create application directories: {e}");
    }

    let pref_dir_cloned = pref_dir.clone();
    let data_dir_cloned = data_dir.clone();
    app.register_asset_source(
        "pref",
        AssetSourceBuilder::default()
            .with_reader(move || Box::new(FileAssetReader::new(&pref_dir)))
            .with_writer(move |create_root| Some(Box::new(FileAssetWriter::new(&pref_dir_cloned, create_root)))),
    )
    .register_asset_source(
        "data",
        AssetSourceBuilder::default()
            .with_reader(move || Box::new(FileAssetReader::new(&data_dir)))
            .with_writer(move |create_root| Some(Box::new(FileAssetWriter::new(&data_dir_cloned, create_root)))),
    );
}

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<AssetLoadingTracker>()
        .add_systems(OnEnter(GameState::AssetLoading), (CharacterTextures::init, TileTextures::init))
        .add_systems(
            Update,
            track_asset_loading
                .run_if(in_state(GameState::AssetLoading))
                .before(ProgressSystems::UpdateTransitions),
        );
}
