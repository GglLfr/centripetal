use crate::{
    PIXELS_PER_UNIT,
    logic::{
        CameraQuery, InGameState, Ldtk, LdtkEntityField, LdtkLayer, LdtkLayerData, LdtkLevel,
        LdtkTiles, LdtkWorld,
    },
    prelude::*,
};

#[derive(Debug, Clone, Resource, TypePath, Serialize, Deserialize, Deref, DerefMut)]
pub struct LoadLevel(pub String);

#[derive(Debug, Clone, Component)]
#[require(LevelSpawned, Transform, Visibility)]
pub struct Level {
    pub id: String,
    pub handle: Handle<LdtkLevel>,
}

#[derive(Debug, Copy, Clone, Component, Deref, DerefMut)]
pub struct LevelBounds(pub Vec2);

#[derive(Debug, Clone, Default, Component, Deref, DerefMut)]
pub struct LevelLayers(pub HashMap<String, Entity>);

#[derive(Debug, Clone, Default, Component, Deref, DerefMut)]
pub struct LevelEntities(pub HashMap<Uuid, Entity>);
impl LevelEntities {
    pub fn get(&self, uuid: Uuid) -> Result<Entity, &'static str> {
        self.0.get(&uuid).copied().ok_or("Entity not found!")
    }
}

#[derive(Debug, Copy, Clone, Component, Default)]
pub struct LevelSpawned(bool);

#[derive(Debug, Copy, Clone, Default, Component)]
pub struct LevelUnload;

#[derive(Debug, Clone, Component)]
pub struct LevelLayer {
    pub id: String,
}

impl LevelLayer {
    pub const ENTITIES: &'static str = "entities";
    pub const TILES_MAIN: &'static str = "tiles_main";
}

#[derive(Debug, Clone, Component)]
#[require(Transform, Visibility, Fields)]
pub struct LevelEntity {
    pub id: String,
    pub iid: Uuid,
}

#[derive(Debug, Clone, Default, Component, Deref, DerefMut)]
pub struct Fields(pub HashMap<String, LdtkEntityField>);

#[derive(Debug, Display, Error)]
pub enum FieldError {
    NotFound,
    WrongType,
}

impl Fields {
    pub fn int(&self, name: impl AsRef<str>) -> Result<u32, FieldError> {
        self.get(name.as_ref())
            .map(|f| match f {
                LdtkEntityField::Int(value) => Ok(*value),
                _ => Err(FieldError::WrongType),
            })
            .unwrap_or(Err(FieldError::NotFound))
    }

    pub fn float(&self, name: impl AsRef<str>) -> Result<f32, FieldError> {
        self.get(name.as_ref())
            .map(|f| match f {
                LdtkEntityField::Float(value) => Ok(*value),
                _ => Err(FieldError::WrongType),
            })
            .unwrap_or(Err(FieldError::NotFound))
    }

    pub fn bool(&self, name: impl AsRef<str>) -> Result<bool, FieldError> {
        self.get(name.as_ref())
            .map(|f| match f {
                LdtkEntityField::Bool(value) => Ok(*value),
                _ => Err(FieldError::WrongType),
            })
            .unwrap_or(Err(FieldError::NotFound))
    }

    pub fn string(&self, name: impl AsRef<str>) -> Result<&str, FieldError> {
        self.get(name.as_ref())
            .map(|f| match f {
                LdtkEntityField::String(value) => Ok(value.as_ref()),
                _ => Err(FieldError::WrongType),
            })
            .unwrap_or(Err(FieldError::NotFound))
    }

    pub fn point(&self, name: impl AsRef<str>) -> Result<UVec2, FieldError> {
        self.get(name.as_ref())
            .map(|f| match f {
                LdtkEntityField::Point(value) => Ok(*value),
                _ => Err(FieldError::WrongType),
            })
            .unwrap_or(Err(FieldError::NotFound))
    }

    pub fn point_px(&self, name: impl AsRef<str>) -> Result<UVec2, FieldError> {
        self.point(name)
            .map(|p| p * PIXELS_PER_UNIT + PIXELS_PER_UNIT / 2)
    }
}

#[derive(Debug, Copy, Clone, Component)]
pub struct LevelTile {
    pub id: u32,
}

#[derive(Debug, Copy, Clone, Component)]
pub struct LevelIntCell {
    pub value: u32,
    pub x: u32,
    pub y: u32,
}

pub trait FromLevel: 'static {
    type Param: 'static + SystemParam;
    type Data: 'static + QueryData;

    fn from_level(
        e: EntityCommands,
        fields: &Fields,
        param: SystemParamItem<Self::Param>,
        data: QueryItem<Self::Data>,
    ) -> Result;
}

pub trait FromLevelEntity: 'static {
    type Param: 'static + SystemParam;
    type Data: 'static + QueryData;

    fn from_level_entity(
        e: EntityCommands,
        fields: &Fields,
        param: &mut SystemParamItem<Self::Param>,
        data: QueryItem<Self::Data>,
    ) -> Result;
}

pub trait FromLevelIntCell: 'static {
    type Param: 'static + SystemParam;
    type Data: 'static + QueryData;

    fn from_level_int_cell(
        e: EntityCommands,
        cell: &LevelIntCell,
        param: &mut SystemParamItem<Self::Param>,
        data: QueryItem<Self::Data>,
    ) -> Result;
}

#[derive(Debug, Clone, Default, Resource)]
pub struct RegisteredLevels(HashMap<String, SystemId<In<Entity>, Result>>);
impl RegisteredLevels {
    pub fn register<T: FromLevel>(&mut self, world: &mut World, level: impl Into<String>) {
        fn spawn<T: FromLevel>(
            In(e): In<Entity>,
            mut commands: Commands,
            param: StaticSystemParam<T::Param>,
            mut query: Query<(Entity, &Fields, T::Data)>,
        ) -> Result {
            let (e, fields, data) = query.get_mut(e)?;
            T::from_level(commands.entity(e), fields, param.into_inner(), data)
        }

        self.0
            .insert(level.into(), world.register_system(spawn::<T>));
    }
}

#[derive(Debug, Clone, Default, Resource)]
pub struct RegisteredLevelEntities(HashMap<String, SystemId<InRef<'static, [Entity]>, Result>>);
impl RegisteredLevelEntities {
    pub fn register<T: FromLevelEntity>(
        &mut self,
        world: &mut World,
        identifier: impl Into<String>,
    ) {
        fn spawn<T: FromLevelEntity>(
            InRef(e): InRef<[Entity]>,
            mut commands: Commands,
            mut param: StaticSystemParam<T::Param>,
            mut query: Query<(Entity, &Fields, T::Data)>,
        ) -> Result {
            let mut query = query.iter_many_mut(e);
            while let Some((e, fields, data)) = query.fetch_next() {
                T::from_level_entity(commands.entity(e), fields, &mut param, data)?;
            }

            Ok(())
        }

        self.0
            .insert(identifier.into(), world.register_system(spawn::<T>));
    }
}

#[derive(Debug, Clone, Default, Resource)]
pub struct RegisteredLevelIntCells(
    HashMap<String, HashMap<u32, SystemId<InRef<'static, [Entity]>, Result>>>,
);
impl RegisteredLevelIntCells {
    pub fn register<T: FromLevelIntCell>(
        &mut self,
        world: &mut World,
        layer: impl AsRef<str>,
        value: u32,
    ) {
        fn spawn<T: FromLevelIntCell>(
            InRef(e): InRef<[Entity]>,
            mut commands: Commands,
            mut param: StaticSystemParam<T::Param>,
            mut query: Query<(Entity, &LevelIntCell, T::Data)>,
        ) -> Result {
            let mut query = query.iter_many_mut(e);
            while let Some((e, cell, data)) = query.fetch_next() {
                T::from_level_int_cell(commands.entity(e), cell, &mut param, data)?;
            }

            Ok(())
        }

        self.0
            .entry_ref(layer.as_ref())
            .or_default()
            .insert(value, world.register_system(spawn::<T>));
    }
}

pub trait LevelApp {
    fn register_level<T: FromLevel>(&mut self, level: impl Into<String>) -> &mut Self;

    fn register_level_entity<T: FromLevelEntity>(
        &mut self,
        identifier: impl Into<String>,
    ) -> &mut Self;

    fn register_level_int_cell<T: FromLevelIntCell>(
        &mut self,
        layer: impl AsRef<str>,
        value: u32,
    ) -> &mut Self;
}

impl LevelApp for App {
    fn register_level<T: FromLevel>(&mut self, level: impl Into<String>) -> &mut Self {
        self.world_mut()
            .resource_scope(|world, mut entities: Mut<RegisteredLevels>| {
                entities.register::<T>(world, level)
            });
        self
    }

    fn register_level_entity<T: FromLevelEntity>(
        &mut self,
        identifier: impl Into<String>,
    ) -> &mut Self {
        self.world_mut()
            .resource_scope(|world, mut entities: Mut<RegisteredLevelEntities>| {
                entities.register::<T>(world, identifier)
            });
        self
    }

    fn register_level_int_cell<T: FromLevelIntCell>(
        &mut self,
        layer: impl AsRef<str>,
        value: u32,
    ) -> &mut Self {
        self.world_mut()
            .resource_scope(|world, mut entities: Mut<RegisteredLevelIntCells>| {
                entities.register::<T>(world, layer, value)
            });
        self
    }
}

pub fn handle_load_level_begin(
    mut commands: Commands,
    next: Option<Res<LoadLevel>>,
    server: Res<AssetServer>,
    world: LdtkWorld,
    mut state: ResMut<NextState<InGameState>>,
    to_be_unloaded: Query<(Entity, &Level), Without<LevelUnload>>,
) -> Result {
    let Some(next) = next
        .as_ref()
        .and_then(|next| next.is_changed().then_some(next.as_str()))
    else {
        return Ok(());
    };

    let mut identity = false;
    for (e, level) in &to_be_unloaded {
        if level.id == next {
            warn!("Tried reloading level {next}, ignoring!");
            identity = true;
        } else {
            commands.entity(e).insert(LevelUnload);
        }
    }

    if !identity {
        let handle = server.load(
            world
                .levels
                .get(next)
                .ok_or_else(|| format!("Level {next} not found"))?,
        );

        commands.spawn(Level {
            id: next.into(),
            handle,
        });

        state.set(InGameState::Loading);
    }

    Ok(())
}

pub fn handle_load_level_progress(
    mut commands: Commands,
    mut camera: CameraQuery<&mut Camera>,
    tracker: ProgressEntry<InGameState>,
    server: Res<AssetServer>,
    level_handle: Query<(Entity, &Level, &mut LevelSpawned), Without<LevelUnload>>,
    levels: Res<Assets<LdtkLevel>>,
    world: LdtkWorld,
) -> Result {
    let (e, level, mut spawned) = level_handle.single_inner()?;
    let mut done = false;
    match (
        server.load_state(&level.handle),
        server.recursive_dependency_load_state(&level.handle),
    ) {
        (LoadState::NotLoaded, ..) => Err("The level's asset handle is dropped")?,
        (LoadState::Loading, ..) | (.., RecursiveDependencyLoadState::Loading) => {}
        (LoadState::Loaded, RecursiveDependencyLoadState::NotLoaded) => {
            Err("The level's asset dependency handle is dropped")?
        }
        (LoadState::Loaded, RecursiveDependencyLoadState::Loaded) => done = true,
        (LoadState::Failed(e), ..) | (.., RecursiveDependencyLoadState::Failed(e)) => Err(e)?,
    }

    if done && !std::mem::replace(&mut spawned.0, true) {
        let level_asset = levels.get(&level.handle).ok_or("Level asset unloaded")?;
        camera.clear_color = ClearColorConfig::Custom(level_asset.bg_color);

        let mut level_layers = LevelLayers::default();
        let mut level_entities = LevelEntities::default();

        commands.entity(e).insert((
            LevelBounds(uvec2(level_asset.width_px, level_asset.height_px).as_vec2()),
            Fields(level_asset.fields.clone()),
        ));

        for layer in &level_asset.layers {
            let mut layer_entity = commands.spawn((
                LevelLayer {
                    id: layer.id.clone(),
                },
                Transform::default(),
                Visibility::default(),
            ));

            match &layer.data {
                LdtkLayerData::Entities(entities) => {
                    layer_entity.with_children(|layer_children| {
                        for e in entities {
                            level_entities.insert(
                                e.iid,
                                layer_children
                                    .spawn((
                                        LevelEntity {
                                            id: e.id.clone(),
                                            iid: e.iid,
                                        },
                                        Fields(e.fields.clone()),
                                        Transform::from_translation(
                                            e.grid_position_px.as_vec2().extend(0.),
                                        ),
                                    ))
                                    .id(),
                            );
                        }
                    });
                }
                LdtkLayerData::IntGrid { grid, tiles } => {
                    layer_entity.with_children(|layer_children| {
                        for &val in grid {
                            layer_children.spawn(LevelIntCell {
                                value: val.value,
                                x: val.x,
                                y: val.y,
                            });
                        }
                    });

                    if let Some(tiles) = tiles {
                        spawn_tilemap(&mut layer_entity, tiles, layer, world.get())?;
                    }
                }
            }

            let layer_entity = layer_entity.id();
            commands.entity(e).add_child(layer_entity);
            level_layers.insert(layer.id.clone(), layer_entity);
        }

        commands.entity(e).insert((level_layers, level_entities));
    }

    tracker.set_progress(done as u32, 1);
    Ok(())
}

fn spawn_tilemap(
    layer_entity: &mut EntityCommands,
    tiles: &LdtkTiles,
    layer: &LdtkLayer,
    world: &Ldtk,
) -> Result {
    let layer_entity_id = layer_entity.id();
    let tileset = world
        .tilesets
        .get(&tiles.tileset)
        .ok_or_else(|| format!("Tileset {} not found", tiles.tileset))?;
    let size = TilemapSize {
        x: layer.width,
        y: layer.height,
    };

    let mut storage = TileStorage::empty(size);
    layer_entity.with_children(|layer_children| {
        for &tile in &tiles.tiles {
            let tile_pos = tile.grid_position_px / layer.grid_size;
            let tile_pos = TilePos::new(tile_pos.x, tile_pos.y);

            storage.set(
                &tile_pos,
                layer_children
                    .spawn((
                        LevelTile { id: tile.id },
                        TileBundle {
                            position: tile_pos,
                            texture_index: TileTextureIndex(
                                (tile.tileset_position_px.y * tileset.width
                                    + tile.tileset_position_px.x)
                                    / tileset.tile_size,
                            ),
                            tilemap_id: TilemapId(layer_entity_id),
                            visible: default(),
                            flip: default(),
                            color: TileColor(Srgba::new(1., 1., 1., tile.alpha).into()),
                            old_position: TilePosOld(tile_pos),
                            sync: SyncToRenderWorld,
                        },
                    ))
                    .id(),
            );
        }
    });

    layer_entity.insert(TilemapBundle {
        grid_size: TilemapGridSize {
            x: layer.grid_size as f32,
            y: layer.grid_size as f32,
        },
        map_type: TilemapType::Square,
        size,
        spacing: TilemapSpacing::zero(),
        storage,
        texture: TilemapTexture::Single(tiles.tileset_image.clone_weak()),
        tile_size: TilemapTileSize {
            x: tileset.tile_size as f32,
            y: tileset.tile_size as f32,
        },
        transform: default(),
        global_transform: default(),
        render_settings: default(),
        visibility: default(),
        inherited_visibility: default(),
        view_visibility: default(),
        frustum_culling: default(),
        material: MaterialTilemapHandle(Handle::<StandardTilemapMaterial>::default()),
        sync: SyncToRenderWorld,
        anchor: default(),
    });

    Ok(())
}

pub fn handle_load_level_end(
    world: &mut World,
    state: &mut SystemState<(
        Res<RegisteredLevels>,
        Res<RegisteredLevelEntities>,
        Res<RegisteredLevelIntCells>,
        Query<(&LevelLayer, &Children, &ChildOf), Added<LevelLayer>>,
        Query<&Level>,
        Query<(Entity, &LevelEntity)>,
        Query<(Entity, &LevelIntCell)>,
        Query<Entity, With<LevelUnload>>,
    )>,
    mut level_targets: Local<HashSet<(SystemId<In<Entity>, Result>, Entity)>>,
    mut entity_targets: Local<HashMap<SystemId<InRef<[Entity]>, Result>, Vec<Entity>>>,
    mut int_cell_targets: Local<HashMap<SystemId<InRef<[Entity]>, Result>, Vec<Entity>>>,
    mut unload_levels: Local<Vec<Entity>>,
) -> Result {
    let (
        level_creators,
        entity_creators,
        int_cell_creators,
        spawned_layers,
        levels,
        entities,
        int_cells,
        to_be_unloaded,
    ) = state.get_mut(world);

    for (layer, layer_children, child_of) in &spawned_layers {
        if let Ok(level) = levels.get(child_of.parent())
            && let Some(&creator) = level_creators.0.get(&level.id)
        {
            level_targets.insert((creator, child_of.parent()));
        }

        for (entity_id, entity) in entities.iter_many(layer_children) {
            if let Some(&create) = entity_creators.0.get(&entity.id) {
                entity_targets.entry(create).or_default().push(entity_id);
            }
        }

        if let Some(creators) = int_cell_creators.0.get(&layer.id) {
            for (cell_id, cell) in int_cells.iter_many(layer_children) {
                if let Some(&create) = creators.get(&cell.value) {
                    int_cell_targets.entry(create).or_default().push(cell_id);
                }
            }
        }
    }

    // Unload previous levels at the very last so some asset handles could be shared.
    unload_levels.extend(to_be_unloaded.into_iter());
    for unload in unload_levels.drain(..) {
        world.try_despawn(unload)?;
    }

    for (&id, entities) in &mut entity_targets {
        world.run_system_with(id, entities.drain(..).as_slice())??;
    }

    for (&id, int_cells) in &mut int_cell_targets {
        world.run_system_with(id, int_cells.drain(..).as_slice())??;
    }

    for (id, level) in level_targets.drain() {
        world.run_system_with(id, level)??;
    }

    world.flush();
    Ok(())
}
