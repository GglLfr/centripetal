use crate::{
    GameState, ProgressFor, ProgressSystems,
    math::Transform2d,
    prelude::*,
    saves::SaveData,
    util::IteratorExt,
    world::{Tile, Tilemap, TilesetImage},
};

#[derive(Reflect, Resource, Debug, Clone, Deref)]
#[reflect(Resource, Debug, Clone)]
pub struct LevelCollectionRef(Arc<LevelCollection>);

#[derive(Reflect, Debug)]
#[reflect(Debug)]
pub struct LevelCollection {
    tilesets: HashMap<u32, Tileset>,
    level_paths: HashMap<Uuid, AssetPath<'static>>,
}

impl LevelCollection {
    pub fn level_path(&self, uuid: Uuid) -> Option<&AssetPath<'static>> {
        self.level_paths.get(&uuid)
    }
}

impl Asset for LevelCollection {}
impl VisitAssetDependencies for LevelCollection {
    fn visit_dependencies(&self, visit: &mut impl FnMut(UntypedAssetId)) {
        for tileset in self.tilesets.values() {
            tileset.visit_dependencies(visit);
        }
    }
}

#[derive(Reflect, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TileProperty {
    Emissive,
}

#[derive(Reflect, Debug)]
#[reflect(Debug)]
pub struct Tileset {
    image: Handle<TilesetImage>,
    properties: HashMap<TileProperty, Vec<u32>>,
    cell_size: UVec2,
    grid_size: u32,
}

impl VisitAssetDependencies for Tileset {
    fn visit_dependencies(&self, visit: &mut impl FnMut(UntypedAssetId)) {
        visit(self.image.id().untyped());
    }
}

pub struct LevelCollectionLoader;
impl AssetLoader for LevelCollectionLoader {
    type Asset = LevelCollection;
    type Settings = ();
    type Error = BevyError;

    #[expect(non_snake_case, reason = "LDtk naming scheme")]
    async fn load(&self, reader: &mut dyn Reader, _: &Self::Settings, load_context: &mut LoadContext<'_>) -> Result<Self::Asset, Self::Error> {
        #[derive(Deserialize)]
        struct Repr {
            defs: DefsRepr,
            levels: Vec<LevelPathRepr>,
        }

        #[derive(Deserialize)]
        struct DefsRepr {
            tilesets: Vec<TilesetRepr>,
        }

        #[derive(Deserialize)]
        struct TilesetRepr {
            uid: u32,
            relPath: String,
            tileGridSize: u32,
            __cWid: u32,
            __cHei: u32,
            enumTags: Vec<EnumRepr>,
        }

        #[derive(Deserialize)]

        struct EnumRepr {
            enumValueId: String,
            tileIds: Vec<u32>,
        }

        #[derive(Deserialize)]
        struct LevelPathRepr {
            iid: Uuid,
            externalRelPath: String,
        }

        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;

        let repr = serde_json::from_slice::<Repr>(&bytes)?;
        Ok(LevelCollection {
            tilesets: repr.defs.tilesets.into_iter().try_map_into_default(|tileset| {
                let grid_size = tileset.tileGridSize;
                let tileset_path = load_context.asset_path().resolve_embed(&tileset.relPath)?;
                Ok::<_, BevyError>((tileset.uid, Tileset {
                    image: load_context
                        .loader()
                        .with_settings(move |size: &mut u32| *size = grid_size)
                        .load(tileset_path),
                    properties: tileset.enumTags.into_iter().try_map_into_default(|tag| {
                        Ok::<_, BevyError>((
                            match tag.enumValueId.as_str() {
                                "emissive" => TileProperty::Emissive,
                                unknown => Err(format!("Unknown tile propertiy `{unknown}`"))?,
                            },
                            tag.tileIds,
                        ))
                    })?,
                    cell_size: uvec2(tileset.__cWid, tileset.__cHei),
                    grid_size,
                }))
            })?,
            level_paths: repr
                .levels
                .into_iter()
                .try_map_into_default(|repr| Ok::<_, BevyError>((repr.iid, load_context.asset_path().resolve_embed(&repr.externalRelPath)?)))?,
        })
    }

    fn extensions(&self) -> &[&str] {
        &["ldtk"]
    }
}

#[derive(Reflect, Asset, Debug)]
pub struct Level {
    pub data: SaveData,
}

pub struct LevelLoader {
    collection: LevelCollectionRef,
    server: AssetServer,
}

impl AssetLoader for LevelLoader {
    type Asset = Level;
    type Settings = ();
    type Error = BevyError;

    #[expect(non_snake_case, reason = "LDtk naming scheme")]
    async fn load(&self, reader: &mut dyn Reader, _: &Self::Settings, _: &mut LoadContext<'_>) -> Result<Self::Asset, Self::Error> {
        #[derive(Deserialize)]
        struct Repr {
            layerInstances: Vec<LayerInstanceRepr>,
        }

        #[derive(Deserialize)]
        struct LayerInstanceRepr {
            __cWid: u32,
            __cHei: u32,
            __gridSize: u32,
            __pxTotalOffsetX: u32,
            __pxTotalOffsetY: u32,
            #[serde(flatten)]
            data: LayerDataRepr,
        }

        #[derive(Deserialize)]
        #[serde(tag = "__type")]
        enum LayerDataRepr {
            Tiles {
                __tilesetDefUid: u32,
                gridTiles: Vec<TileInstanceRepr>,
            },
        }

        #[derive(Deserialize)]
        struct TileInstanceRepr {
            px: [u32; 2],
            t: u32,
        }

        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;

        let repr = serde_json::from_slice::<Repr>(&bytes)?;
        let mut data = SaveData::default();

        // It's okay to allocate "random" entities as they will be mapped before spawning anyway.
        let mut current_index = EntityRow::from_raw_u32(0).unwrap();
        let mut new_entity = || match EntityRow::from_raw_u32(current_index.index() + 1) {
            Some(next_index) => {
                let index = mem::replace(&mut current_index, next_index);
                Ok(Entity::from_row_and_generation(index, EntityGeneration::FIRST))
            }
            None => Err("Too many entities"),
        };

        for (i, layer) in repr.layerInstances.into_iter().enumerate() {
            match layer.data {
                LayerDataRepr::Tiles { __tilesetDefUid, gridTiles } => {
                    let tileset = self
                        .collection
                        .tilesets
                        .get(&__tilesetDefUid)
                        .ok_or_else(|| format!("Missing tileset `{__tilesetDefUid}`"))?;
                    let base_path = tileset.image.path().ok_or("Missing tileset asset path")?;

                    let tilemap_entity = new_entity()?;
                    let tilemap_size = uvec2(layer.__cWid, layer.__cHei);
                    let mut tilemap = Tilemap::new(tilemap_size);

                    for tile in gridTiles {
                        let tile_entity = new_entity()?;
                        let tx = tile.t % tileset.cell_size.x;
                        let ty = tile.t / tileset.cell_size.x;

                        let path = base_path.clone().with_label(format!("{tx},{ty}"));
                        let tile = Tile::new(
                            tilemap_entity,
                            uvec2(tile.px[0] / layer.__gridSize, layer.__cHei - tile.px[1] / layer.__gridSize),
                            match self.server.get_path_ids(&path).into_iter().find_map(|id| id.try_typed().ok()) {
                                Some(id) => {
                                    if let AssetId::Index { index, .. } = id {
                                        data.asset_paths.entry(id.untyped().type_id()).or_default().insert(index, path);
                                    }
                                    id
                                }
                                None => Err(format!("Missing `{path}` required for loading tile"))?,
                            },
                        );

                        if tilemap
                            .tile_mut(tile.pos)
                            .ok_or_else(|| format!("Tile ({}) out of bounds ({})", tile.pos, tilemap_size))?
                            .replace(tile_entity)
                            .is_some()
                        {
                            Err(format!("Duplicated tile at ({})", tile.pos))?
                        }

                        tilemap.change_chunk(tile.pos);
                        data.entities.insert(tile_entity, vec![Box::new(tile)]);
                    }

                    data.entities.insert(tilemap_entity, vec![
                        Box::new(tilemap),
                        Box::new(Transform2d {
                            translation: vec3(layer.__pxTotalOffsetX as f32, layer.__pxTotalOffsetY as f32, i as f32 * 0.1),
                            ..default()
                        }),
                    ]);
                }
            }
        }

        Ok(Level { data })
    }

    fn extensions(&self) -> &[&str] {
        &["ldtkl"]
    }
}

#[derive(Resource)]
struct LevelCollectionHandle(Handle<LevelCollection>);

fn init_level_collection(mut commands: Commands, server: Res<AssetServer>) {
    server.register_loader(LevelCollectionLoader);
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
            commands.insert_resource(collection.clone());
            server.register_loader(LevelLoader {
                collection,
                server: server.clone(),
            });

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
        .init_asset::<Level>()
        .register_asset_reflect::<LevelCollection>()
        .register_asset_reflect::<Level>()
        .preregister_asset_loader::<LevelCollectionLoader>(&["ldtk"])
        .preregister_asset_loader::<LevelLoader>(&["ldtkl"])
        .add_systems(Startup, init_level_collection)
        .add_systems(
            Update,
            query_level_collection
                .run_if(in_state(GameState::AssetLoading))
                .before(ProgressSystems::UpdateTransitions),
        );
}
