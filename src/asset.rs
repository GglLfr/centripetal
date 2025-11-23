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
            $vis fn init(mut commands: Commands, server: Res<AssetServer>, mut known_assets: ResMut<KnownAssets>) {
                $(
                    let $asset_name = server.load($asset_path);
                    known_assets.id_to_path.insert($asset_name.id().untyped(), AssetPath::from($asset_path));
                    if let Some(old_id) = known_assets.path_to_id.insert(AssetPath::from($asset_path), $asset_name.id().untyped())
                        && old_id != $asset_name.id().untyped()
                    {
                        known_assets.id_to_path.remove(&old_id);
                    }
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

/// Asset IDS in this collection may be (de)serialized across runs, since they originate from a
/// defined collection. Be cautious when refactoring asset paths, e.g. moving files around to
/// another directory, as this will invalidate save files that refer to those paths.
#[derive(Resource, Debug, Default)]
pub struct KnownAssets {
    id_to_path: HashMap<UntypedAssetId, AssetPath<'static>>,
    path_to_id: HashMap<AssetPath<'static>, UntypedAssetId>,
}

impl KnownAssets {
    pub fn id_to_path(&self) -> &HashMap<UntypedAssetId, AssetPath<'static>> {
        &self.id_to_path
    }

    pub fn path_to_id(&self) -> &HashMap<AssetPath<'static>, UntypedAssetId> {
        &self.path_to_id
    }
}

fn track_asset_loading(progress: ProgressFor<GameState>, known_assets: Res<KnownAssets>, server: Res<AssetServer>) -> Result {
    let mut current = 0;
    for &id in known_assets.id_to_path.keys() {
        current += match (server.load_state(id), server.recursive_dependency_load_state(id)) {
            (LoadState::Failed(e), ..) | (.., RecursiveDependencyLoadState::Failed(e)) => Err(e)?,
            (LoadState::NotLoaded, ..) | (.., RecursiveDependencyLoadState::NotLoaded) => Err("Some asset handles got dropped")?,
            (LoadState::Loaded, RecursiveDependencyLoadState::Loaded) => 2,
            (LoadState::Loaded, ..) | (.., RecursiveDependencyLoadState::Loaded) => 1,
            (LoadState::Loading, RecursiveDependencyLoadState::Loading) => 0,
        };
    }

    progress.update([current, known_assets.id_to_path.len() * 2]);
    Ok(())
}

pub trait MapAssetIds: 'static {
    fn visit_asset_ids(&self, visitor: &mut dyn FnMut(UntypedAssetId));

    fn map_asset_ids(&mut self, mapper: &mut dyn AssetIdMapper);
}

pub trait AssetIdMapper {
    fn map(&mut self, id: UntypedAssetId) -> UntypedAssetId;
}

#[derive(Clone, Copy)]
pub struct ReflectMapAssetIds {
    visit_asset_ids: fn(&dyn PartialReflect, &mut dyn FnMut(UntypedAssetId)),
    map_asset_ids: fn(&mut dyn PartialReflect, &mut dyn AssetIdMapper),
}

impl ReflectMapAssetIds {
    pub fn visit_asset_ids(&self, target: &dyn PartialReflect, visitor: &mut dyn FnMut(UntypedAssetId)) {
        (self.visit_asset_ids)(target, visitor)
    }

    pub fn map_asset_ids(&self, target: &mut dyn PartialReflect, mapper: &mut dyn AssetIdMapper) {
        (self.map_asset_ids)(target, mapper)
    }
}

impl<T: MapAssetIds> FromType<T> for ReflectMapAssetIds {
    fn from_type() -> Self {
        Self {
            visit_asset_ids: |target, visitor| {
                let target = target.try_downcast_ref::<T>().expect("Wrong type provided");
                target.visit_asset_ids(visitor);
            },
            map_asset_ids: |target, mapper| {
                let target = target.try_downcast_mut::<T>().expect("Wrong type provided");
                target.map_asset_ids(mapper);
            },
        }
    }
}

impl<T: Asset> MapAssetIds for AssetId<T> {
    fn visit_asset_ids(&self, visitor: &mut dyn FnMut(UntypedAssetId)) {
        visitor(self.untyped())
    }

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
    app.init_resource::<KnownAssets>()
        .add_systems(OnEnter(GameState::AssetLoading), (CharacterTextures::init, TileTextures::init))
        .add_systems(
            Update,
            track_asset_loading
                .run_if(in_state(GameState::AssetLoading))
                .before(ProgressSystems::UpdateTransitions),
        );
}
