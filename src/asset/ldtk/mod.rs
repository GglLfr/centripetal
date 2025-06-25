use bevy::{
    asset::{AssetPath, UntypedAssetId, VisitAssetDependencies, uuid::Uuid},
    platform::collections::HashMap,
    prelude::*,
};

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
    pub width: u32,
    pub height: u32,
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
