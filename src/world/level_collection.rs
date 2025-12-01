use crate::{
    GameState, ProgressFor, ProgressSystems,
    prelude::*,
    render::atlas::{AtlasInfo, AtlasRegion, PageInfo},
    util::IteratorExt,
    world::TileProperty,
};

mod sealed {
    use super::*;

    pub trait Sealed {}
    impl<T: 'static + PartialReflect + Hash + PartialEq + Eq> Sealed for T {}
}

pub trait WorldEnum: 'static + PartialReflect + sealed::Sealed {
    fn eq(&self, other: &dyn WorldEnum) -> bool;

    fn hash(&self, state: &mut dyn Hasher);
}

impl<T: 'static + PartialReflect + Hash + PartialEq + Eq> WorldEnum for T {
    fn eq(&self, other: &dyn WorldEnum) -> bool {
        (other as &dyn PartialReflect)
            .try_downcast_ref::<Self>()
            .is_some_and(|other| self == other)
    }

    fn hash(&self, mut state: &mut dyn Hasher) {
        self.hash(&mut state);
    }
}

impl Debug for dyn WorldEnum {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.debug(f)
    }
}

impl Hash for dyn WorldEnum {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.hash(state);
    }
}

impl PartialEq for dyn WorldEnum {
    fn eq(&self, other: &Self) -> bool {
        self.eq(other)
    }
}

impl Eq for dyn WorldEnum {}

#[derive(Default, Debug, Clone)]
pub struct WorldEnums {
    pub by_name: HashMap<String, fn(&str) -> Result<Box<dyn WorldEnum>>>,
    pub by_index: HashMap<u32, String>,
}

impl WorldEnums {
    fn with<T: WorldEnum + for<'de> Deserialize<'de>>(mut self, name: impl Into<String>) -> Self {
        self.by_name.insert(name.into(), |variant| {
            struct ErrorWrapper(BevyError);
            impl std::error::Error for ErrorWrapper {}
            impl Debug for ErrorWrapper {
                fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                    Debug::fmt(&self.0, f)
                }
            }

            impl fmt::Display for ErrorWrapper {
                fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                    fmt::Display::fmt(&self.0, f)
                }
            }

            impl de::Error for ErrorWrapper {
                fn custom<T: fmt::Display>(msg: T) -> Self {
                    Self(format!("{msg}").into())
                }
            }

            Ok(Box::new(
                T::deserialize(de::value::StrDeserializer::new(variant)).map_err(|ErrorWrapper(e)| e)?,
            ))
        });
        self
    }
}

#[derive(Reflect, Debug)]
#[reflect(Debug)]
pub struct LevelCollection {
    #[reflect(ignore)]
    pub enums: WorldEnums,
    pub layers: HashMap<u32, Layer>,
    pub tilesets: HashMap<u32, Tileset>,
    pub level_paths: HashMap<String, PathBuf>,
    #[reflect(ignore)]
    pub source: AssetSourceId<'static>,
}

impl Asset for LevelCollection {}
impl VisitAssetDependencies for LevelCollection {
    fn visit_dependencies(&self, visit: &mut impl FnMut(UntypedAssetId)) {
        for tileset in self.tilesets.values() {
            tileset.visit_dependencies(visit);
        }
    }
}

#[derive(Reflect, Debug)]
#[reflect(Debug)]
pub struct Layer {
    pub parallax: Vec2,
    pub parallax_scale: bool,
}

#[derive(Reflect, Debug)]
#[reflect(Debug)]
pub struct Tileset {
    pub region: Handle<AtlasRegion>,
    pub tiles: HashMap<UVec2, Handle<AtlasRegion>>,
    #[reflect(ignore)]
    pub properties: HashMap<Box<dyn WorldEnum>, Vec<u32>>,
    pub cell_size: UVec2,
    pub grid_size: u32,
}

impl VisitAssetDependencies for Tileset {
    fn visit_dependencies(&self, visit: &mut impl FnMut(UntypedAssetId)) {
        visit(self.region.id().untyped());
        for tile in self.tiles.values() {
            visit(tile.id().untyped());
        }
    }
}

pub struct LevelCollectionLoader {
    enums: WorldEnums,
}

impl AssetLoader for LevelCollectionLoader {
    type Asset = LevelCollection;
    type Settings = ();
    type Error = BevyError;

    async fn load(&self, reader: &mut dyn Reader, _: &Self::Settings, load_context: &mut LoadContext<'_>) -> Result<Self::Asset, Self::Error> {
        #[derive(Deserialize)]
        struct Repr {
            defs: DefsRepr,
            levels: Vec<LevelPathRepr>,
        }

        #[derive(Deserialize)]
        struct DefsRepr {
            layers: Vec<LayerRepr>,
            enums: Vec<EnumDefRepr>,
            tilesets: Vec<TilesetRepr>,
        }

        #[derive(Deserialize)]
        #[expect(non_snake_case, reason = "LDtk naming scheme")]
        struct LayerRepr {
            uid: u32,
            parallaxFactorX: f32,
            parallaxFactorY: f32,
            parallaxScaling: bool,
        }

        #[derive(Deserialize)]
        struct EnumDefRepr {
            identifier: String,
            uid: u32,
            values: Vec<EnumValueRepr>,
        }

        #[derive(Deserialize)]
        struct EnumValueRepr {
            id: String,
        }

        #[derive(Deserialize)]
        #[expect(non_snake_case, reason = "LDtk naming scheme")]
        struct TilesetRepr {
            uid: u32,
            relPath: String,
            tileGridSize: u32,
            __cWid: u32,
            __cHei: u32,
            tagsSourceEnumUid: Option<u32>,
            enumTags: Vec<EnumTagRepr>,
            identifier: String,
        }

        #[derive(Deserialize)]
        #[expect(non_snake_case, reason = "LDtk naming scheme")]
        struct EnumTagRepr {
            enumValueId: String,
            tileIds: Vec<u32>,
        }

        #[derive(Deserialize)]
        #[expect(non_snake_case, reason = "LDtk naming scheme")]
        struct LevelPathRepr {
            identifier: String,
            externalRelPath: String,
        }

        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;

        let repr = serde_json::from_slice::<Repr>(&bytes)?;
        let mut enums = self.enums.clone();

        for enum_def in repr.defs.enums {
            let ident = enum_def.identifier;
            let &enum_ctor = enums.by_name.get(&ident).ok_or_else(|| format!("Enum `{ident}` doesn't exist"))?;

            for value in enum_def.values {
                _ = enum_ctor(&value.id)?
            }

            enums.by_index.insert(enum_def.uid, ident);
        }

        let mut tilesets = HashMap::new();
        for tileset in repr.defs.tilesets {
            let grid_size = tileset.tileGridSize;
            let tileset_path = load_context.asset_path().resolve_embed(&tileset.relPath)?;

            let region = load_context.loader().immediate().load::<AtlasRegion>(&tileset_path).await?;
            let region_ref = region.get();

            if region_ref.rect.size() % grid_size != UVec2::ZERO {
                Err(format!("Tileset image size ({}) isn't a multiple of {grid_size}", region_ref.rect.size()))?
            }

            let mut tiles = HashMap::new();
            for y in 0..region_ref.rect.size().y / grid_size {
                for x in 0..region_ref.rect.size().x / grid_size {
                    tiles.insert(
                        uvec2(x, y),
                        load_context.add_labeled_asset(format!("{}#{x},{y}", tileset.identifier), AtlasRegion {
                            info: AtlasInfo {
                                page: PageInfo {
                                    texture: region_ref.page.texture.clone(),
                                    texture_size: region_ref.page.texture_size.clone(),
                                },
                                rect: URect {
                                    min: region_ref.rect.min + uvec2(x, y) * grid_size,
                                    max: region_ref.rect.min + uvec2(x + 1, y + 1) * grid_size,
                                },
                            },
                        }),
                    );
                }
            }

            tilesets.insert(tileset.uid, Tileset {
                region: load_context.add_loaded_labeled_asset(tileset.identifier, region),
                tiles,
                properties: tileset.enumTags.into_iter().try_map_into_default(|tag| {
                    let enum_index = tileset.tagsSourceEnumUid.ok_or("`tagsSourceEnumUid` required for `enumTags`")?;
                    let &enum_ctor = enums
                        .by_index
                        .get(&enum_index)
                        .and_then(|enum_name| enums.by_name.get(enum_name))
                        .ok_or_else(|| format!("Missing enum {enum_index}"))?;

                    Ok::<_, BevyError>((enum_ctor(&tag.enumValueId)?, tag.tileIds))
                })?,
                cell_size: uvec2(tileset.__cWid, tileset.__cHei),
                grid_size,
            });
        }

        Ok(LevelCollection {
            enums,
            layers: repr
                .defs
                .layers
                .into_iter()
                .map(|layer| {
                    (layer.uid, Layer {
                        parallax: vec2(layer.parallaxFactorX, layer.parallaxFactorY),
                        parallax_scale: layer.parallaxScaling,
                    })
                })
                .collect(),
            tilesets,
            level_paths: repr.levels.into_iter().try_map_into_default(|repr| {
                Ok::<_, BevyError>((
                    repr.identifier,
                    load_context.asset_path().resolve_embed(&repr.externalRelPath)?.path().into(),
                ))
            })?,
            source: load_context.asset_path().source().clone_owned(),
        })
    }

    fn extensions(&self) -> &[&str] {
        &["ldtk"]
    }
}

#[derive(Resource, Clone)]
pub struct LevelCollectionRef(Arc<LevelCollection>);
impl Deref for LevelCollectionRef {
    type Target = LevelCollection;

    fn deref(&self) -> &Self::Target {
        &*self.0
    }
}

#[derive(Resource)]
struct LevelCollectionHandle(Handle<LevelCollection>);

fn init_level_collection(mut commands: Commands, server: Res<AssetServer>) {
    commands.insert_resource(LevelCollectionHandle(server.load("levels/world.ldtk")));
}

fn query_level_collection(
    mut commands: Commands,
    progress: ProgressFor<GameState>,
    server: Res<AssetServer>,
    mut assets: ResMut<Assets<LevelCollection>>,
    handle: Option<Res<LevelCollectionHandle>>,
) -> Result {
    let Some(handle) = handle else {
        progress.update([2, 2]);
        return Ok(())
    };

    match (server.load_state(&handle.0), server.recursive_dependency_load_state(&handle.0)) {
        (LoadState::Failed(e), ..) | (.., RecursiveDependencyLoadState::Failed(e)) => Err(e)?,
        (LoadState::NotLoaded, ..) | (.., RecursiveDependencyLoadState::NotLoaded) => Err("Level collection handle got dropped")?,
        (LoadState::Loaded, RecursiveDependencyLoadState::Loaded) => {
            let collection = LevelCollectionRef(Arc::new(assets.remove(&handle.0).ok_or("Level collection unexpectedly removed")?));
            commands.insert_resource(collection);

            commands.remove_resource::<LevelCollectionHandle>();
            progress.update([2, 2]);
        }
        (LoadState::Loaded, ..) | (.., RecursiveDependencyLoadState::Loaded) => progress.update([1, 2]),
        (LoadState::Loading, RecursiveDependencyLoadState::Loading) => progress.update([0, 2]),
    };
    Ok(())
}

pub(super) fn plugin(app: &mut App) {
    app.init_asset::<LevelCollection>()
        .register_asset_reflect::<LevelCollection>()
        .register_asset_loader(LevelCollectionLoader {
            enums: WorldEnums::default().with::<TileProperty>("tile_properties"),
        })
        .add_systems(Startup, init_level_collection)
        .add_systems(
            Update,
            query_level_collection
                .run_if(in_state(GameState::AssetLoading))
                .before(ProgressSystems::UpdateTransitions),
        );
}
