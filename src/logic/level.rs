use std::borrow::Cow;

use bevy::{
    asset::{LoadState, RecursiveDependencyLoadState},
    ecs::{
        query::{QueryData, QueryItem},
        system::{StaticSystemParam, SystemId, SystemParam, SystemParamItem, SystemState},
    },
    platform::collections::HashMap,
    prelude::*,
    render::sync_world::SyncToRenderWorld,
};
use bevy_ecs_tilemap::prelude::*;
use derive_more::{Display, Error};
use iyes_progress::ProgressEntry;

use crate::{
    asset::{
        LdtkWorld,
        ldtk::{Ldtk, LdtkEntityField, LdtkLayer, LdtkLayerData, LdtkLevel, LdtkTiles},
    },
    logic::InGameState,
};

#[derive(Debug, Clone, Event, Deref, DerefMut)]
pub struct LoadLevelEvent(pub Cow<'static, str>);

#[derive(Debug, Clone, Component, Deref, DerefMut)]
#[require(LevelSpawned, Transform, Visibility)]
pub struct LevelHandle(pub Handle<LdtkLevel>);

#[derive(Debug, Copy, Clone, Component, Deref, DerefMut)]
pub struct LevelBounds(pub Vec2);

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
#[require(Transform, Visibility)]
pub struct LevelEntity {
    pub id: String,
    pub fields: HashMap<String, LdtkEntityField>,
}

#[derive(Debug, Display, Error)]
pub enum LevelEntityFieldError {
    NotFound,
    WrongType,
}

impl LevelEntity {
    pub fn int(&self, name: impl AsRef<str>) -> Result<u32, LevelEntityFieldError> {
        self.fields
            .get(name.as_ref())
            .map(|f| match f {
                LdtkEntityField::Int(value) => Ok(*value),
                _ => Err(LevelEntityFieldError::WrongType),
            })
            .unwrap_or(Err(LevelEntityFieldError::NotFound))
    }

    pub fn float(&self, name: impl AsRef<str>) -> Result<f32, LevelEntityFieldError> {
        self.fields
            .get(name.as_ref())
            .map(|f| match f {
                LdtkEntityField::Float(value) => Ok(*value),
                _ => Err(LevelEntityFieldError::WrongType),
            })
            .unwrap_or(Err(LevelEntityFieldError::NotFound))
    }

    pub fn bool(&self, name: impl AsRef<str>) -> Result<bool, LevelEntityFieldError> {
        self.fields
            .get(name.as_ref())
            .map(|f| match f {
                LdtkEntityField::Bool(value) => Ok(*value),
                _ => Err(LevelEntityFieldError::WrongType),
            })
            .unwrap_or(Err(LevelEntityFieldError::NotFound))
    }

    pub fn string(&self, name: impl AsRef<str>) -> Result<&str, LevelEntityFieldError> {
        self.fields
            .get(name.as_ref())
            .map(|f| match f {
                LdtkEntityField::String(value) => Ok(value.as_ref()),
                _ => Err(LevelEntityFieldError::WrongType),
            })
            .unwrap_or(Err(LevelEntityFieldError::NotFound))
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

pub trait FromLevelEntity: 'static {
    type Param: 'static + SystemParam;
    type Data: 'static + QueryData;

    fn from_level_entity(
        e: EntityCommands,
        entity: &LevelEntity,
        param: &mut SystemParamItem<Self::Param>,
        data: QueryItem<Self::Data>,
    ) -> Result;
}

pub trait FromLevelIntCell: 'static {
    type Param: 'static + SystemParam;
    type Data: 'static + QueryData;

    fn from_level_int_cell(
        e: EntityCommands,
        entity: &LevelIntCell,
        param: &mut SystemParamItem<Self::Param>,
        data: QueryItem<Self::Data>,
    ) -> Result;
}

#[derive(Debug, Clone, Default, Resource)]
pub struct LevelEntities(HashMap<String, HashMap<String, SystemId<InRef<'static, [Entity]>, Result>>>);
impl LevelEntities {
    pub fn register<T: FromLevelEntity>(&mut self, world: &mut World, layer: impl AsRef<str>, identifier: impl AsRef<str>) {
        fn spawn<T: FromLevelEntity>(
            InRef(e): InRef<[Entity]>,
            mut commands: Commands,
            mut param: StaticSystemParam<T::Param>,
            mut query: Query<(Entity, &LevelEntity, T::Data)>,
        ) -> Result {
            let mut query = query.iter_many_mut(e);
            while let Some((e, entity, data)) = query.fetch_next() {
                T::from_level_entity(commands.entity(e), entity, &mut param, data)?;
            }

            Ok(())
        }

        self.0
            .entry_ref(layer.as_ref())
            .or_default()
            .insert(identifier.as_ref().into(), world.register_system(spawn::<T>));
    }
}

#[derive(Debug, Clone, Default, Resource)]
pub struct LevelIntCells(HashMap<String, HashMap<u32, SystemId<InRef<'static, [Entity]>, Result>>>);
impl LevelIntCells {
    pub fn register<T: FromLevelIntCell>(&mut self, world: &mut World, layer: impl AsRef<str>, value: u32) {
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
    fn register_level_entity<T: FromLevelEntity>(
        &mut self,
        layer: impl AsRef<str>,
        identifier: impl AsRef<str>,
    ) -> &mut Self;

    fn register_level_int_cell<T: FromLevelIntCell>(&mut self, layer: impl AsRef<str>, value: u32) -> &mut Self;
}

impl LevelApp for App {
    fn register_level_entity<T: FromLevelEntity>(
        &mut self,
        layer: impl AsRef<str>,
        identifier: impl AsRef<str>,
    ) -> &mut Self {
        self.world_mut()
            .resource_scope(|world, mut entities: Mut<LevelEntities>| entities.register::<T>(world, layer, identifier));
        self
    }

    fn register_level_int_cell<T: FromLevelIntCell>(&mut self, layer: impl AsRef<str>, value: u32) -> &mut Self {
        self.world_mut()
            .resource_scope(|world, mut entities: Mut<LevelIntCells>| entities.register::<T>(world, layer, value));
        self
    }
}

pub fn handle_load_level_event(
    mut commands: Commands,
    mut events: EventReader<LoadLevelEvent>,
    server: Res<AssetServer>,
    world: LdtkWorld,
    mut state: ResMut<NextState<InGameState>>,
    to_be_unloaded: Query<Entity, (With<LevelHandle>, Without<LevelUnload>)>,
) -> Result {
    let Some(event) = events.read().last() else { return Ok(()) };
    for e in &to_be_unloaded {
        debug!("Unloading level entity {e}");
        commands.entity(e).insert(LevelUnload);
    }

    debug!(
        "Loading level {} as entity {}!",
        **event,
        commands
            .spawn(LevelHandle(
                server.load(
                    world
                        .levels
                        .get(event.as_ref())
                        .ok_or_else(|| format!("Level {} not found", event.as_ref()))?,
                ),
            ))
            .id()
    );

    state.set(InGameState::Loading);
    Ok(())
}

pub fn handle_load_level_progress(
    mut commands: Commands,
    tracker: ProgressEntry<InGameState>,
    server: Res<AssetServer>,
    mut level_handles: Query<(Entity, &LevelHandle, &mut LevelSpawned), Without<LevelUnload>>,
    levels: Res<Assets<LdtkLevel>>,
    world: LdtkWorld,
) -> Result {
    let mut all_done = 0;
    let mut all_total = 0;

    for (e, handle, mut spawned) in &mut level_handles {
        let mut done = false;
        match (
            server.load_state(handle.id()),
            server.recursive_dependency_load_state(handle.id()),
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
            debug!("Loaded level entity {e}!");

            let level = levels.get(handle.id()).ok_or("Level asset unloaded")?;
            commands.insert_resource(ClearColor(level.bg_color));

            commands
                .entity(e)
                .insert(LevelBounds(uvec2(level.width_px, level.height_px).as_vec2()));

            for layer in &level.layers {
                let mut layer_entity = commands.spawn((
                    LevelLayer { id: layer.id.clone() },
                    Transform::default(),
                    Visibility::default(),
                ));

                match &layer.data {
                    LdtkLayerData::Entities(entities) => {
                        layer_entity.with_children(|layer_children| {
                            for e in entities {
                                layer_children.spawn((
                                    LevelEntity {
                                        id: e.id.clone(),
                                        fields: e.fields.clone(),
                                    },
                                    Transform::from_translation(e.grid_position_px.as_vec2().extend(0.)),
                                ));
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
            }
        }

        all_done += if done { 1 } else { 0 };
        all_total += 1;
    }

    tracker.set_progress(all_done, all_total);
    Ok(())
}

fn spawn_tilemap(layer_entity: &mut EntityCommands, tiles: &LdtkTiles, layer: &LdtkLayer, world: &Ldtk) -> Result {
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
                    .spawn((LevelTile { id: tile.id }, TileBundle {
                        position: tile_pos,
                        texture_index: TileTextureIndex(
                            (tile.tileset_position_px.y * tileset.width + tile.tileset_position_px.x) / tileset.tile_size,
                        ),
                        tilemap_id: TilemapId(layer_entity_id),
                        visible: default(),
                        flip: default(),
                        color: TileColor(Srgba::new(1., 1., 1., tile.alpha).into()),
                        old_position: TilePosOld(tile_pos),
                        sync: SyncToRenderWorld,
                    }))
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
        Res<LevelEntities>,
        Res<LevelIntCells>,
        Query<(&LevelLayer, &Children), Added<LevelLayer>>,
        Query<(Entity, &LevelEntity)>,
        Query<(Entity, &LevelIntCell)>,
        ResMut<NextState<InGameState>>,
        Query<Entity, With<LevelUnload>>,
    )>,
    mut entity_targets: Local<HashMap<SystemId<InRef<[Entity]>, Result>, Vec<Entity>>>,
    mut int_cell_targets: Local<HashMap<SystemId<InRef<[Entity]>, Result>, Vec<Entity>>>,
    mut unload_levels: Local<Vec<Entity>>,
) -> Result {
    let (entity_creators, int_cell_creators, spawned_layers, entities, int_cells, mut state, to_be_unloaded) =
        state.get_mut(world);

    for (layer, layer_children) in &spawned_layers {
        if let Some(creators) = entity_creators.0.get(&layer.id) {
            for (entity_id, entity) in entities.iter_many(layer_children) {
                if let Some(&create) = creators.get(&entity.id) {
                    entity_targets.entry(create).or_default().push(entity_id);
                }
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
    state.set(InGameState::Resumed);
    unload_levels.extend(to_be_unloaded);
    for e in unload_levels.drain(..) {
        world.try_despawn(e)?;
    }

    for (&id, entities) in &mut entity_targets {
        world.run_system_with(id, entities.drain(..).as_slice())??;
    }

    for (&id, int_cells) in &mut int_cell_targets {
        world.run_system_with(id, int_cells.drain(..).as_slice())??;
    }

    world.flush();
    debug!("All levels have been loaded!");

    Ok(())
}
