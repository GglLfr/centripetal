use crate::{
    GameState, ProgressFor, ProgressSystems,
    math::Transform2d,
    prelude::*,
    util::{IteratorExt, async_bridge::AsyncBridge},
    world::{LevelCollectionRef, Tile, Tilemap, TilemapParallax, WorldEnum},
};

#[derive(Reflect, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize)]
#[serde(rename_all = "snake_case")]
#[reflect(Debug, Clone, PartialEq, Hash)]
pub enum TileProperty {
    Emissive,
    Collision,
}

#[derive(Component, Debug, Deref, DerefMut)]
#[component(immutable)]
pub struct TilemapProperties {
    pub tiles: HashMap<TileProperty, HashSet<u32>>,
}

#[derive(Component, Debug, Clone, Copy, Deref, DerefMut)]
#[component(immutable)]
pub struct TileId(pub u32);

#[derive(Resource, Default)]
pub enum LoadLevel {
    #[default]
    None,
    Pending(String),
}

impl LoadLevel {
    pub fn load(&mut self, level_identifier: impl Into<String>) {
        *self = Self::Pending(level_identifier.into());
    }
}

#[derive(Debug)]
pub struct EntityFields {
    pub map: HashMap<String, EntityField>,
}

#[derive(Debug)]
pub enum EntityField {
    Int(i64),
    Float(f64),
    String(String),
    Path(PathBuf),
    Enum(Arc<dyn WorldEnum>),
    GridPoint(UVec2),
    Tileset { id: u32, rect: URect },
    Entity { entity: Uuid, layer: Uuid, level: Uuid, world: Uuid },
}

#[derive(Message, Debug)]
pub struct EntityCreate {
    pub identifier: String,
    pub entity: Entity,
    pub fields: EntityFields,
    pub bounds: Rect,
    pub tile_pos: UVec2,
}

pub trait MessageReaderEntityExt {
    fn created(&mut self, id: &str) -> impl Iterator<Item = &EntityCreate>;
}

impl MessageReaderEntityExt for MessageReader<'_, '_, EntityCreate> {
    fn created(&mut self, id: &str) -> impl Iterator<Item = &EntityCreate> {
        self.read().filter(move |msg| msg.identifier == id)
    }
}

#[derive(Message, Debug)]
pub enum LayerCreate {
    Entities { identifier: String, entities: Vec<Entity> },
    Tiles { entity: Entity, kind: TileLayerKind },
}

/// Tile layer identifier, defined in reverse order as specified in the LDtkl file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TileLayerKind {
    /// `tiles_back`, should not create colliders.
    Back,
    /// `tiles_main`, should create colliders.
    Main,
    /// `tiles_front`, should not create colliders.
    Front,
}

pub trait MessageReaderLayerExt {
    fn tiles(&mut self, tile_kind: TileLayerKind) -> Option<Entity>;

    fn tiles_back(&mut self) -> Option<Entity> {
        self.tiles(TileLayerKind::Back)
    }

    fn tiles_main(&mut self) -> Option<Entity> {
        self.tiles(TileLayerKind::Main)
    }

    fn tiles_front(&mut self) -> Option<Entity> {
        self.tiles(TileLayerKind::Front)
    }
}

impl MessageReaderLayerExt for MessageReader<'_, '_, LayerCreate> {
    fn tiles(&mut self, tile_kind: TileLayerKind) -> Option<Entity> {
        self.read()
            .filter_map(|layer| match layer {
                &LayerCreate::Tiles { entity, kind } if kind == tile_kind => Some(entity),
                _ => None,
            })
            .last()
    }
}

#[derive(Resource)]
enum LoadLevelProgress {
    Pending(String),
    Running(Duration, Task<Result<LoadLevelOutput>>),
    Done,
}

fn load_level_transition(mut commands: Commands, mut load_level: ResMut<LoadLevel>, mut state: ResMut<NextState<GameState>>) {
    let LoadLevel::Pending(level_identifier) = mem::take(&mut *load_level) else { return };
    commands.insert_resource(LoadLevelProgress::Pending(level_identifier));
    state.set(GameState::LevelLoading);
}

fn load_level(
    progress: ProgressFor<GameState>,
    time: Res<Time>,
    server: Res<AssetServer>,
    mut load_level: ResMut<LoadLevelProgress>,
    collection: Res<LevelCollectionRef>,
    mut entity_creation_writer: MessageWriter<EntityCreate>,
    mut layer_creation_writer: MessageWriter<LayerCreate>,
    bridge: Res<AsyncBridge>,
) -> Result {
    let LoadLevelProgress::Running(started, task) = (match &mut *load_level {
        LoadLevelProgress::Pending(level_identifier) => {
            info!("Begin level loading of {level_identifier}...");

            let level_identifier = mem::take(level_identifier);
            *load_level = LoadLevelProgress::Running(
                time.elapsed(),
                AsyncComputeTaskPool::get().spawn(load_level_task(level_identifier, &server, &collection, &bridge)),
            );
            &mut *load_level
        }
        this @ LoadLevelProgress::Running(..) => this,
        LoadLevelProgress::Done => return Ok(()),
    }) else {
        unreachable!("Above match invariably sets `LoadLevelProgress` to `Running(..)`")
    };

    match check_ready(task) {
        Some(Ok(output)) => {
            info!("Level loading done! Took {}ms.", (time.elapsed() - *started).as_secs_f32() * 1_000.);

            entity_creation_writer.write_batch(output.entity_creation);
            layer_creation_writer.write_batch(output.layer_creation);
            *load_level = LoadLevelProgress::Done;
            progress.update(true);
            Ok(())
        }
        Some(Err(e)) => {
            error!("Level loading failed! See below for details.");
            Err(e)
        }
        None => {
            progress.update(false);
            Ok(())
        }
    }
}

#[derive(Default)]
struct LoadLevelOutput {
    entity_creation: Vec<EntityCreate>,
    layer_creation: Vec<LayerCreate>,
}

fn load_level_task(
    level_identifier: String,
    server: &AssetServer,
    collection: &LevelCollectionRef,
    bridge: &AsyncBridge,
) -> impl Future<Output = Result<LoadLevelOutput>> + use<> {
    #[derive(Deserialize)]
    #[expect(non_snake_case, reason = "LDtk naming scheme")]
    struct Repr {
        layerInstances: Vec<LayerInstanceRepr>,
    }

    #[derive(Deserialize)]
    #[expect(non_snake_case, reason = "LDtk naming scheme")]
    struct LayerInstanceRepr {
        __identifier: String,
        __cWid: u32,
        __cHei: u32,
        __gridSize: u32,
        layerDefUid: u32,
        #[serde(flatten)]
        data: LayerDataRepr,
    }

    #[derive(Deserialize)]
    #[serde(tag = "__type")]
    #[expect(non_snake_case, reason = "LDtk naming scheme")]
    enum LayerDataRepr {
        Entities {
            entityInstances: Vec<EntityInstanceRepr>,
        },
        Tiles {
            __tilesetDefUid: u32,
            gridTiles: Vec<TileInstanceRepr>,
        },
    }

    #[derive(Deserialize)]
    #[expect(non_snake_case, reason = "LDtk naming scheme")]
    struct EntityInstanceRepr {
        __identifier: String,
        __grid: [u32; 2],
        px: [u32; 2],
        __pivot: [f32; 2],
        width: u32,
        height: u32,
        fieldInstances: Vec<FieldInstanceRepr>,
    }

    #[derive(Deserialize)]
    struct FieldInstanceRepr {
        __identifier: String,
        __type: String,
        __value: serde_json::Value,
    }

    #[derive(Deserialize)]
    struct TileInstanceRepr {
        px: [u32; 2],
        t: u32,
    }

    let server = server.clone();
    let collection = collection.clone();
    let ctx = bridge.ctx();
    async move {
        let mut output = LoadLevelOutput::default();

        let path = collection
            .level_paths
            .get(&level_identifier)
            .ok_or_else(|| format!("Missing level `{level_identifier}`"))?;
        let source = server.get_source(&collection.source)?;

        let mut bytes = Vec::new();
        Reader::read_to_end(&mut source.reader().read(path).await?, &mut bytes).await?;

        let repr = serde_json::from_slice::<Repr>(&bytes)?;
        let mut commands = ctx.commands();

        let mut used_names = HashSet::new();
        for (i, layer) in repr.layerInstances.into_iter().rev().enumerate() {
            if !used_names.insert(layer.__identifier.clone()) {
                Err(format!("Duplicate layer {}", layer.__identifier))?
            }

            let layer_def = collection
                .layers
                .get(&layer.layerDefUid)
                .ok_or_else(|| format!("Missing layer definition `{}`", layer.layerDefUid))?;

            match layer.data {
                LayerDataRepr::Entities { entityInstances } => {
                    let entities = commands.spawn_many(entityInstances.len() as u32).await?;
                    output.layer_creation.push(LayerCreate::Entities {
                        identifier: layer.__identifier,
                        entities: entities.as_slice().into(),
                    });

                    for (instance, entity) in entityInstances.into_iter().zip(entities) {
                        let size = uvec2(instance.width, instance.height).as_vec2();
                        let bounds_start = uvec2(instance.px[0], layer.__cHei * layer.__gridSize - instance.px[1]).as_vec2()
                            - vec2(instance.__pivot[0], 1. - instance.__pivot[1]) * size;

                        output.entity_creation.push(EntityCreate {
                            entity,
                            identifier: instance.__identifier,
                            fields: EntityFields {
                                map: instance.fieldInstances.into_iter().try_flat_map_into_default(|field| {
                                    Ok::<_, BevyError>(match field.__type.as_str() {
                                        "Int" => field.__value.as_i64().map(EntityField::Int),
                                        "Float" => field.__value.as_f64().map(EntityField::Float),
                                        "String" => field.__value.as_str().map(|s| EntityField::String(s.into())),
                                        "FilePath" => field.__value.as_str().map(|s| EntityField::Path(s.into())),
                                        // TODO GridPoint, Tileset, Entity
                                        other => {
                                            if let Some(enum_name) = other.strip_prefix("LocalEnum.") {
                                                let &enum_ctor = collection
                                                    .enums
                                                    .by_name
                                                    .get(enum_name)
                                                    .ok_or_else(|| format!("Enum `{enum_name}` doesn't exist"))?;

                                                let enum_variant = field.__value.as_str().ok_or("Expected string")?;
                                                Some(enum_ctor(enum_variant).map(EntityField::Enum)?)
                                            } else {
                                                Err(format!("Unknown field type `{other}`"))?
                                            }
                                        }
                                    })
                                    .map(|opt| opt.map(|f| (field.__identifier, f)))
                                })?,
                            },
                            bounds: Rect {
                                min: bounds_start,
                                max: bounds_start + size,
                            },
                            tile_pos: uvec2(instance.__grid[0], layer.__cHei - instance.__grid[1]),
                        });
                    }
                }
                LayerDataRepr::Tiles { __tilesetDefUid, gridTiles } => {
                    let tileset = collection
                        .tilesets
                        .get(&__tilesetDefUid)
                        .ok_or_else(|| format!("Missing tileset {__tilesetDefUid}"))?;
                    let mut entities = commands.spawn_many(gridTiles.len() as u32 + 1).await?.into_iter();

                    let tilemap_entity = entities.next().expect("Non-zero integer was provided; the entity must exist");
                    commands.entity(tilemap_entity).insert((
                        Tilemap::new(layer.__gridSize as f32, uvec2(layer.__cWid, layer.__cHei)),
                        TilemapProperties {
                            tiles: tileset.properties.iter().try_map_into_default(|(key, value)| {
                                Ok::<_, BevyError>((
                                    *(key.as_ref() as &dyn PartialReflect)
                                        .try_downcast_ref()
                                        .ok_or("Tile layers must use tilesets with `tile_properties` enum")?,
                                    value.clone(),
                                ))
                            })?,
                        },
                    ));

                    let kind = match layer.__identifier.as_str() {
                        "tiles_back" => TileLayerKind::Back,
                        "tiles_main" => TileLayerKind::Main,
                        "tiles_front" => TileLayerKind::Front,
                        unknown => Err(format!("Unknown tile layer `{unknown}`"))?,
                    };
                    output.layer_creation.push(LayerCreate::Tiles {
                        entity: tilemap_entity,
                        kind,
                    });

                    for (tile, tile_entity) in gridTiles.into_iter().zip(entities) {
                        let tileset_pos = uvec2(tile.t % tileset.cell_size.x, tile.t / tileset.cell_size.x);
                        let tile_pos = uvec2(tile.px[0] / layer.__gridSize, layer.__cHei - tile.px[1] / layer.__gridSize - 1);

                        commands.entity(tile_entity).insert((
                            Tile::new(
                                tilemap_entity,
                                tile_pos,
                                tileset
                                    .tiles
                                    .get(&tileset_pos)
                                    .ok_or_else(|| format!("No tileset tile defined at ({tileset_pos})"))?,
                            ),
                            TileId(tile.t),
                        ));
                    }

                    commands.entity(tilemap_entity).insert((
                        Transform2d {
                            translation: vec3(0., 0., i as f32 * 0.1),
                            ..default()
                        },
                        match kind {
                            TileLayerKind::Main => {
                                if layer_def.parallax != Vec2::ZERO {
                                    Err("`tiles_main` must not have parallax effects!")?
                                }

                                TilemapParallax {
                                    factor: Vec2::ZERO,
                                    scale: false,
                                }
                            }
                            _ => TilemapParallax {
                                factor: layer_def.parallax,
                                scale: layer_def.parallax_scale,
                            },
                        },
                    ));
                }
            }
        }

        commands.submit().await?;
        Ok(output)
    }
}

fn tiles_main_created(
    mut commands: Commands,
    mut tiles: MessageReader<LayerCreate>,
    tilemap_query: Query<(&Tilemap, &TilemapProperties)>,
    tile_query: Query<&TileId>,
) {
    if let Some(e) = tiles.tiles_main()
        && let Ok((tilemap, properties)) = tilemap_query.get(e)
        && let Some(collisions) = properties.get(&TileProperty::Collision)
    {
        commands.entity(e).insert((
            RigidBody::Static,
            Collider::voxels(
                Vec2::splat(tilemap.grid_size()),
                &*tilemap
                    .iter_tiles()
                    .flat_map(|(pos, tile)| {
                        tile_query
                            .get(tile)
                            .is_ok_and(|&tile| collisions.contains(&*tile))
                            .then_some(pos.as_ivec2())
                    })
                    .collect::<Box<_>>(),
            ),
            #[cfg(feature = "dev")]
            DebugRender::none(),
        ));
    }
}

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LevelSystems {
    Load,
    SpawnEntities,
}

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<LoadLevel>()
        .add_message::<EntityCreate>()
        .add_message::<LayerCreate>()
        .configure_sets(
            Update,
            (LevelSystems::Load, LevelSystems::SpawnEntities)
                .chain()
                .before(ProgressSystems::UpdateTransitions)
                .run_if(in_state(GameState::LevelLoading)),
        )
        .add_systems(PreUpdate, load_level_transition.run_if(not(in_state(GameState::LevelLoading))))
        .add_systems(
            Update,
            (
                load_level.in_set(LevelSystems::Load),
                tiles_main_created.in_set(LevelSystems::SpawnEntities),
            ),
        );
}
