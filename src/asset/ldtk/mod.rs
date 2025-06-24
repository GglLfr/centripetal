use bevy::{
    asset::{AssetPath, UntypedAssetId, VisitAssetDependencies, uuid::Uuid},
    platform::collections::HashMap,
    prelude::*,
};
use bevy_ecs_tilemap::prelude::*;

mod loader;

pub use loader::*;

#[derive(Debug, Asset, TypePath)]
pub struct Ldtk {
    pub iid: Uuid,
    pub levels: HashMap<Uuid, AssetPath<'static>>,
    pub level_identifiers: HashMap<String, Uuid>,
    pub tilesets: HashMap<u32, LdtkTileset>,
}

#[derive(Debug)]
pub struct LdtkTileset {
    pub tile_size: u32,
}

#[derive(Debug, TypePath)]
pub struct LdtkLevel {
    pub iid: Uuid,
    pub bg_color: Color,
    pub layers: Vec<LdtkLayer>,
}

impl Asset for LdtkLevel {}
impl VisitAssetDependencies for LdtkLevel {
    fn visit_dependencies(&self, visit: &mut impl FnMut(UntypedAssetId)) {
        for layer in &self.layers {
            match &layer.data {
                LdtkLayerData::IntGrid { tiles, .. } => {
                    if let Some(tiles) = tiles {
                        visit(tiles.tileset_image.id().untyped());
                    }
                }
            }
        }
    }
}

#[derive(Debug)]
pub struct LdtkLayer {
    pub id: String,
    pub width: u32,
    pub height: u32,
    pub grid_size: u32,
    pub data: LdtkLayerData,
}

#[derive(Debug)]
pub enum LdtkLayerData {
    IntGrid {
        grid: Vec<LdtkIntCell>,
        tiles: Option<LdtkTiles>,
    },
}

#[derive(Debug, Copy, Clone, Component)]
pub struct LdtkIntCell {
    pub value: u32,
    pub x: u32,
    pub y: u32,
}

#[derive(Debug, Clone)]
pub struct LdtkTiles {
    pub tileset: u32,
    pub tileset_image: Handle<Image>,
    pub tiles: Vec<LdtkTile>,
}

#[derive(Debug, Copy, Clone, Component)]
pub struct LdtkTile {
    pub id: u32,
    pub grid_position_px: UVec2,
    pub tileset_position_px: UVec2,
    pub alpha: f32,
}

#[derive(Debug, Copy, Clone, Default)]
pub struct LdtkPlugin;
impl Plugin for LdtkPlugin {
    fn build(&self, app: &mut App) {
        app.init_asset::<Ldtk>()
            .init_asset::<LdtkLevel>()
            .init_asset_loader::<LdtkLoader>()
            .init_asset_loader::<LdtkLevelLoader>();
    }
}

/*
#[derive(Debug, Asset, TypePath)]
pub struct LdtkLevel {
    pub iid: Uuid,
    pub layers: Vec<LdtkLayer>,
}

#[derive(Debug)]
pub struct LdtkLayer {
    pub iid: Uuid,
    pub width: u32,
    pub height: u32,
    pub data: LdtkLayerData,
}

#[derive(Debug)]
pub enum LdtkLayerData {
    IntGrid {
        grid: Vec<LdtkIntCell>,
        tiles: Option<LdtkTiles>,
    },
}

impl LdtkLayerData {
    pub fn insert_to_entity<M: MaterialTilemap>(&self, entity: &mut EntityCommands, material: Handle<M>, root: &Ldtk) {}
}

#[derive(Debug, Copy, Clone, Component)]
pub struct LdtkIntCell {
    pub value: u32,
    pub x: u32,
    pub y: u32,
}

#[derive(Debug, Clone)]
pub struct LdtkTiles {
    pub grid_size: TilemapGridSize,
    pub size: TilemapSize,
    pub tileset: u32,
    pub tiles: Vec<LdtkTile>,
}

#[derive(Debug, Copy, Clone)]
pub struct LdtkTile {
    pub position: TilePos,
    pub texture_index: TileTextureIndex,
}

impl LdtkTiles {
    pub fn insert_to_entity<M: MaterialTilemap>(&self, entity: &mut EntityCommands, material: Handle<M>, root: &Ldtk) {
        let Self {
            grid_size,
            size,
            tileset,
            tiles,
        } = self.clone();

        let mut storage = TileStorage::empty(size);
        entity.with_children(|spawn| {
            let tilemap_entity = spawn.target_entity();
            for tile in tiles {
                let pos = tile.position;
                storage.set(
                    &pos,
                    spawn
                        .spawn(TileBundle {
                            position: pos,
                            texture_index: tile.texture_index,
                            tilemap_id: TilemapId(tilemap_entity),
                            ..default()
                        })
                        .id(),
                );
            }
        });

        let tileset = &root.tilesets[&tileset];
        entity.insert(MaterialTilemapBundle {
            grid_size,
            map_type: TilemapType::Square,
            size,
            spacing: TilemapSpacing::zero(),
            storage,
            texture: TilemapTexture::Single(tileset.image.clone_weak()),
            tile_size: TilemapTileSize {
                x: tileset.tile_size as f32,
                y: tileset.tile_size as f32,
            },
            material: MaterialTilemapHandle(material),
            transform: default(),
            global_transform: default(),
            render_settings: default(),
            visibility: default(),
            inherited_visibility: default(),
            view_visibility: default(),
            frustum_culling: default(),
            sync: SyncToRenderWorld,
            anchor: TilemapAnchor::None,
        });
    }
}

#[derive(Debug, Copy, Clone, Default)]
pub struct LdtkPlugin;
impl Plugin for LdtkPlugin {
    fn build(&self, app: &mut App) {
        app.init_asset::<Ldtk>()
            .init_asset::<LdtkLevel>()
            .init_asset_loader::<LdtkLoader>()
            .init_asset_loader::<LdtkLevelLoader>();
    }
}
*/
