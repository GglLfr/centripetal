use crate::{GameState, ProgressSystems, prelude::*, progress::ProgressFor, render::atlas::AtlasRegion};

macro_rules! define_collection {
    ($(#[$attr:meta])* $vis:vis $name:ident { $($asset_name:ident: $asset_type:path => $asset_path:expr),* }) => {
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
        selene: AtlasRegion => "entities/characters/selene/selene.png"
    }
}

define_collection! {
    pub TileTextures {
        tmp: AtlasRegion => "entities/characters/selene/selene.png"
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
