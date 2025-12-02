use crate::{
    GameState, ProgressFor, ProgressSystems,
    math::Transform2d,
    prelude::*,
    util::{IteratorExt, async_bridge::AsyncBridge},
    world::{LevelCollectionRef, Tile, Tilemap, TilemapParallax, WorldEnum},
};

#[derive(Reflect, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TileProperty {
    Emissive,
}

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
    Enum(Box<dyn WorldEnum>),
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

impl<'w, 's> MessageReaderEntityExt for MessageReader<'w, 's, EntityCreate> {
    fn created(&mut self, id: &str) -> impl Iterator<Item = &EntityCreate> {
        self.read().filter(move |msg| msg.identifier == id)
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
    mut entity_creation_messages: MessageWriter<EntityCreate>,
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

            entity_creation_messages.write_batch(output.entity_creation);
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

        for (i, layer) in repr.layerInstances.into_iter().rev().enumerate() {
            let layer_def = collection
                .layers
                .get(&layer.layerDefUid)
                .ok_or_else(|| format!("Missing layer definition `{}`", layer.layerDefUid))?;

            match layer.data {
                LayerDataRepr::Entities { entityInstances } => {
                    let entities = commands.spawn_many(entityInstances.len() as u32).await?;
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
                    commands.entity(tilemap_entity).insert(Tilemap::new(uvec2(layer.__cWid, layer.__cHei)));

                    for (tile, tile_entity) in gridTiles.into_iter().zip(entities) {
                        let tileset_pos = uvec2(tile.t % tileset.cell_size.x, tile.t / tileset.cell_size.x);
                        commands.entity(tile_entity).insert(Tile::new(
                            tilemap_entity,
                            uvec2(tile.px[0] / layer.__gridSize, layer.__cHei - tile.px[1] / layer.__gridSize),
                            tileset
                                .tiles
                                .get(&tileset_pos)
                                .ok_or_else(|| format!("No tileset tile defined at ({tileset_pos})"))?,
                        ));
                    }

                    commands.entity(tilemap_entity).insert((
                        TilemapParallax {
                            factor: layer_def.parallax,
                            scale: layer_def.parallax_scale,
                        },
                        Transform2d {
                            translation: vec3(0., 0., i as f32 * 0.1),
                            ..default()
                        },
                    ));
                }
            }
        }

        commands.submit().await?;
        Ok(output)
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
        .configure_sets(
            Update,
            (LevelSystems::Load, LevelSystems::SpawnEntities)
                .chain()
                .before(ProgressSystems::UpdateTransitions)
                .run_if(in_state(GameState::LevelLoading)),
        )
        .add_systems(PreUpdate, load_level_transition.run_if(not(in_state(GameState::LevelLoading))))
        .add_systems(Update, load_level.in_set(LevelSystems::Load));
}
