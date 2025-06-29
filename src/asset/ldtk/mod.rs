use bevy::{
    asset::{AssetPath, UntypedAssetId, VisitAssetDependencies},
    platform::collections::HashMap,
    prelude::*,
};

mod loader;

pub use loader::*;

#[derive(Debug, Clone, Asset, TypePath)]
pub struct Ldtk {
    pub levels: HashMap<String, AssetPath<'static>>,
    pub tilesets: HashMap<u32, LdtkTileset>,
}

#[derive(Debug, Clone)]
pub struct LdtkTileset {
    pub tile_size: u32,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, TypePath)]
pub struct LdtkLevel {
    pub bg_color: Color,
    pub width_px: u32,
    pub height_px: u32,
    pub layers: Vec<LdtkLayer>,
}

impl Asset for LdtkLevel {}
impl VisitAssetDependencies for LdtkLevel {
    fn visit_dependencies(&self, visit: &mut impl FnMut(UntypedAssetId)) {
        for layer in &self.layers {
            match &layer.data {
                LdtkLayerData::Entities(..) => {}
                LdtkLayerData::IntGrid { tiles, .. } => {
                    if let Some(tiles) = tiles {
                        visit(tiles.tileset_image.id().untyped());
                    }
                }
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct LdtkLayer {
    pub id: String,
    pub width: u32,
    pub height: u32,
    pub grid_size: u32,
    pub data: LdtkLayerData,
}

#[derive(Debug, Clone)]
pub enum LdtkLayerData {
    Entities(Vec<LdtkEntity>),
    IntGrid {
        grid: Vec<LdtkIntCell>,
        tiles: Option<LdtkTiles>,
    },
}

#[derive(Debug, Clone)]
pub struct LdtkEntity {
    pub id: String,
    pub grid_position_px: UVec2,
    pub fields: HashMap<String, LdtkEntityField>,
}

#[derive(Debug, Clone)]
pub enum LdtkEntityField {
    Int(u32),
    Float(f32),
    Bool(bool),
    String(String),
}

#[derive(Debug, Copy, Clone)]
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

#[derive(Debug, Copy, Clone)]
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
