use std::borrow::Cow;

use bevy::{
    asset::{LoadState, RecursiveDependencyLoadState},
    prelude::*,
    render::sync_world::SyncToRenderWorld,
};
use bevy_ecs_tilemap::prelude::*;
use iyes_progress::ProgressEntry;

use crate::{
    asset::{
        LdtkWorld,
        ldtk::{Ldtk, LdtkLayer, LdtkLayerData, LdtkLevel, LdtkTiles},
    },
    logic::InGameState,
};

#[derive(Debug, Clone, Event, Deref, DerefMut)]
pub struct LoadLevelEvent(pub Cow<'static, str>);

#[derive(Debug, Clone, Component, Deref, DerefMut)]
#[require(LevelSpawned)]
pub struct LevelHandle(pub Handle<LdtkLevel>);

#[derive(Debug, Copy, Clone, Component, Default)]
pub struct LevelSpawned(bool);

#[derive(Debug, Clone, Component)]
pub struct LevelLayer {
    pub id: String,
}

impl LevelLayer {
    pub const TILES_MAIN: &'static str = "tiles_main";
}

#[derive(Debug, Copy, Clone, Component)]
pub struct LevelTile {
    pub id: u32,
}

pub fn handle_load_level_event(
    mut commands: Commands,
    mut events: EventReader<LoadLevelEvent>,
    server: Res<AssetServer>,
    world: LdtkWorld,
    mut state: ResMut<NextState<InGameState>>,
) -> Result {
    let Some(event) = events.read().last() else { return Ok(()) };
    commands.spawn(LevelHandle(
        server.load(
            world
                .levels
                .get(event.as_ref())
                .ok_or_else(|| format!("Level {} not found", event.as_ref()))?,
        ),
    ));

    state.set(InGameState::Loading);
    Ok(())
}

pub fn handle_load_level_progress(
    mut commands: Commands,
    tracker: ProgressEntry<InGameState>,
    server: Res<AssetServer>,
    mut level_handles: Query<(Entity, &LevelHandle, &mut LevelSpawned)>,
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
            let level = levels.get(handle.id()).ok_or("Level asset unloaded")?;
            commands.insert_resource(ClearColor(level.bg_color));

            for layer in &level.layers {
                let mut layer_entity = commands.spawn(LevelLayer { id: layer.id.clone() });
                match &layer.data {
                    LdtkLayerData::Entities => {}
                    LdtkLayerData::IntGrid { grid, tiles } => {
                        layer_entity.with_children(|layer_children| {
                            for &val in grid {
                                layer_children.spawn(val);
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
