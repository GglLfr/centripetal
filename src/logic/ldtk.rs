use std::io;

use bevy::{
    asset::{
        AssetLoader, AssetPath, AsyncReadExt, LoadContext, ParseAssetPathError, RenderAssetUsages,
        UntypedAssetId, VisitAssetDependencies, io::Reader, uuid::Uuid,
    },
    image::ImageLoaderSettings,
    platform::collections::HashMap,
    prelude::*,
};
use blocking::unblock;
use derive_more::{Display, Error, From};
use serde::{
    Deserialize, Deserializer,
    de::{self, Visitor},
};

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
    pub fields: HashMap<String, LdtkEntityField>,
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
    pub iid: Uuid,
    pub grid_position_px: UVec2,
    pub fields: HashMap<String, LdtkEntityField>,
}

#[derive(Debug, Clone)]
pub enum LdtkEntityField {
    Int(u32),
    Float(f32),
    Bool(bool),
    String(String),
    Point(UVec2),
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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Root {
    defs: Definitions,
    levels: Vec<LevelPath>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Definitions {
    tilesets: Vec<Tileset>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Tileset {
    uid: u32,
    tile_grid_size: u32,
    #[serde(rename = "__cWid")]
    width: u32,
    #[serde(rename = "__cHei")]
    height: u32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LevelPath {
    identifier: String,
    external_rel_path: String,
}

fn de_color<'de, D: Deserializer<'de>>(de: D) -> Result<Srgba, D::Error> {
    struct Visit;
    impl<'de> Visitor<'de> for Visit {
        type Value = Srgba;

        fn visit_str<E: de::Error>(self, v: &str) -> Result<Self::Value, E> {
            Srgba::hex(v).map_err(E::custom)
        }

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            write!(formatter, "A hex color starting with #")
        }
    }

    de.deserialize_str(Visit)
}

#[derive(Debug, Display, Error, From)]
pub enum LdtkError {
    #[display("External levels are supposed to be in a subdirectory")]
    #[error(ignore)]
    MissingParentDirectory,
    Path(ParseAssetPathError),
    Json(serde_json::Error),
    Io(io::Error),
}

#[derive(Debug, Copy, Clone, Default)]
pub struct LdtkLoader;
impl AssetLoader for LdtkLoader {
    type Asset = Ldtk;
    type Settings = ();
    type Error = LdtkError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _: &Self::Settings,
        load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let mut file = String::new();
        reader.read_to_string(&mut file).await?;

        let root: Root = unblock(move || serde_json::from_str(&file)).await?;
        let base_path = load_context.asset_path().clone();

        let levels = root
            .levels
            .into_iter()
            .try_fold(HashMap::new(), |mut levels, path| {
                levels.insert(
                    path.identifier,
                    base_path.resolve_embed(&path.external_rel_path)?,
                );
                Ok::<_, Self::Error>(levels)
            })?;

        Ok(Ldtk {
            levels,
            tilesets: root.defs.tilesets.into_iter().try_fold(
                HashMap::new(),
                |mut out, tileset| {
                    out.insert(
                        tileset.uid,
                        LdtkTileset {
                            tile_size: tileset.tile_grid_size,
                            width: tileset.width,
                            height: tileset.height,
                        },
                    );

                    Ok::<_, Self::Error>(out)
                },
            )?,
        })
    }

    fn extensions(&self) -> &[&str] {
        &["ldtk"]
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Level {
    #[serde(rename = "__bgColor", deserialize_with = "de_color")]
    bg_color: Srgba,
    #[serde(rename = "pxWid")]
    width_px: u32,
    #[serde(rename = "pxHei")]
    height_px: u32,
    layer_instances: Vec<LayerInstance>,
    field_instances: Vec<FieldInstance>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LayerInstance {
    #[serde(rename = "__identifier")]
    id: String,
    #[serde(rename = "__cWid")]
    width: u32,
    #[serde(rename = "__cHei")]
    height: u32,
    #[serde(rename = "__gridSize")]
    grid_size: u32,
    #[serde(flatten)]
    data: LayerInstanceData,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all_fields = "camelCase", tag = "__type")]
enum LayerInstanceData {
    Entities {
        entity_instances: Vec<EntityInstance>,
    },
    IntGrid {
        int_grid_csv: Vec<u32>,
        auto_layer_tiles: Option<Vec<TileInstance>>,
        #[serde(rename = "__tilesetDefUid")]
        tileset: Option<u32>,
        #[serde(rename = "__tilesetRelPath")]
        tileset_image: String,
    },
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EntityInstance {
    #[serde(rename = "__identifier")]
    id: String,
    iid: Uuid,
    px: [u32; 2],
    field_instances: Vec<FieldInstance>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FieldInstance {
    #[serde(rename = "__identifier")]
    id: String,
    #[serde(flatten)]
    data: FieldInstanceData,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all_fields = "camelCase", tag = "__type")]
enum FieldInstanceData {
    Int {
        #[serde(rename = "__value")]
        value: Option<u32>,
    },
    Float {
        #[serde(rename = "__value")]
        value: Option<f32>,
    },
    Bool {
        #[serde(rename = "__value")]
        value: Option<bool>,
    },
    String {
        #[serde(rename = "__value")]
        value: Option<String>,
    },
    Point {
        #[serde(rename = "__value")]
        value: Option<Point>,
    },
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Point {
    cx: u32,
    cy: u32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TileInstance {
    a: f32,
    px: [u32; 2],
    src: [u32; 2],
    t: u32,
}

#[derive(Debug, Copy, Clone, Default)]
pub struct LdtkLevelLoader;
impl AssetLoader for LdtkLevelLoader {
    type Asset = LdtkLevel;
    type Settings = ();
    type Error = LdtkError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _: &Self::Settings,
        load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let mut file = String::new();
        reader.read_to_string(&mut file).await?;

        let level: Level = unblock(move || serde_json::from_str(&file)).await?;
        let base_path = load_context
            .asset_path()
            .parent()
            .ok_or(LdtkError::MissingParentDirectory)?;

        let fields = |field_instances: Vec<FieldInstance>| {
            field_instances
                .into_iter()
                .filter_map(|f| {
                    Some((
                        f.id,
                        match f.data {
                            FieldInstanceData::Int { value } => LdtkEntityField::Int(value?),
                            FieldInstanceData::Float { value } => LdtkEntityField::Float(value?),
                            FieldInstanceData::Bool { value } => LdtkEntityField::Bool(value?),
                            FieldInstanceData::String { value } => LdtkEntityField::String(value?),
                            FieldInstanceData::Point { value } => {
                                LdtkEntityField::Point(value.map(|p| uvec2(p.cx, p.cy))?)
                            }
                        },
                    ))
                })
                .collect()
        };

        Ok(LdtkLevel {
            bg_color: level.bg_color.into(),
            width_px: level.width_px,
            height_px: level.height_px,
            fields: fields(level.field_instances),
            layers: level
                .layer_instances
                .into_iter()
                .try_fold(Vec::new(), |mut out, layer| {
                    // Convert Y+ bottom to Y+ top.
                    let top_grid = layer.height - 1;
                    let top_px = layer.height * layer.grid_size;

                    out.push(LdtkLayer {
                        id: layer.id,
                        width: layer.width,
                        height: layer.height,
                        grid_size: layer.grid_size,
                        data: match layer.data {
                            LayerInstanceData::Entities { entity_instances } => {
                                LdtkLayerData::Entities(
                                    entity_instances
                                        .into_iter()
                                        .map(|e| LdtkEntity {
                                            id: e.id,
                                            iid: e.iid,
                                            grid_position_px: uvec2(e.px[0], top_px - e.px[1]),
                                            fields: fields(e.field_instances),
                                        })
                                        .collect(),
                                )
                            }
                            LayerInstanceData::IntGrid {
                                int_grid_csv,
                                auto_layer_tiles,
                                tileset,
                                tileset_image,
                            } => LdtkLayerData::IntGrid {
                                grid: int_grid_csv
                                    .into_iter()
                                    .enumerate()
                                    .map(|(i, value)| LdtkIntCell {
                                        x: i as u32 % layer.width,
                                        y: top_grid - i as u32 / layer.width,
                                        value,
                                    })
                                    .collect(),
                                tiles: auto_layer_tiles
                                    .zip(tileset)
                                    .map(|(tiles, tileset)| {
                                        Ok::<_, LdtkError>(LdtkTiles {
                                            tileset,
                                            tileset_image: load_context
                                                .loader()
                                                .with_settings(
                                                    |settings: &mut ImageLoaderSettings| {
                                                        settings.asset_usage =
                                                            RenderAssetUsages::RENDER_WORLD;
                                                    },
                                                )
                                                .load(base_path.resolve_embed(&tileset_image)?),
                                            tiles: tiles
                                                .into_iter()
                                                .map(|tile| LdtkTile {
                                                    id: tile.t,
                                                    grid_position_px: uvec2(
                                                        tile.px[0],
                                                        top_px - tile.px[1],
                                                    ),
                                                    tileset_position_px: tile.src.into(),
                                                    alpha: tile.a,
                                                })
                                                .collect(),
                                        })
                                    })
                                    .transpose()?,
                            },
                        },
                    });

                    Ok::<_, LdtkError>(out)
                })?,
        })
    }

    fn extensions(&self) -> &[&str] {
        &["ldtkl"]
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
