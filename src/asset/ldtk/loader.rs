use std::io;

use bevy::{
    asset::{AssetLoader, AsyncReadExt, LoadContext, ParseAssetPathError, RenderAssetUsages, io::Reader},
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

use crate::asset::ldtk::{
    Ldtk, LdtkEntity, LdtkEntityField, LdtkIntCell, LdtkLayer, LdtkLayerData, LdtkLevel, LdtkTile, LdtkTiles, LdtkTileset,
};

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

        let levels = root.levels.into_iter().try_fold(HashMap::new(), |mut levels, path| {
            levels.insert(path.identifier, base_path.resolve_embed(&path.external_rel_path)?);
            Ok::<_, Self::Error>(levels)
        })?;

        Ok(Ldtk {
            levels,
            tilesets: root.defs.tilesets.into_iter().try_fold(HashMap::new(), |mut out, tileset| {
                out.insert(tileset.uid, LdtkTileset {
                    tile_size: tileset.tile_grid_size,
                    width: tileset.width,
                    height: tileset.height,
                });

                Ok::<_, Self::Error>(out)
            })?,
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
    layer_instances: Vec<LayerInstance>,
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
        let base_path = load_context.asset_path().parent().ok_or(LdtkError::MissingParentDirectory)?;

        Ok(LdtkLevel {
            bg_color: level.bg_color.into(),
            layers: level.layer_instances.into_iter().try_fold(Vec::new(), |mut out, layer| {
                // Convert Y+ bottom to Y+ top.
                let top_grid = layer.height - 1;
                let top_px = top_grid * layer.grid_size;

                out.push(LdtkLayer {
                    id: layer.id,
                    width: layer.width,
                    height: layer.height,
                    grid_size: layer.grid_size,
                    data: match layer.data {
                        LayerInstanceData::Entities { entity_instances } => LdtkLayerData::Entities(
                            entity_instances
                                .into_iter()
                                .map(|e| LdtkEntity {
                                    id: e.id,
                                    grid_position_px: uvec2(e.px[0], top_px - e.px[1]),
                                    fields: e
                                        .field_instances
                                        .into_iter()
                                        .filter_map(|f| {
                                            Some((f.id, match f.data {
                                                FieldInstanceData::Int { value } => LdtkEntityField::Int(value?),
                                                FieldInstanceData::Float { value } => LdtkEntityField::Float(value?),
                                                FieldInstanceData::Bool { value } => LdtkEntityField::Bool(value?),
                                                FieldInstanceData::String { value } => LdtkEntityField::String(value?),
                                            }))
                                        })
                                        .collect(),
                                })
                                .collect(),
                        ),
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
                                            .with_settings(|settings: &mut ImageLoaderSettings| {
                                                settings.asset_usage = RenderAssetUsages::RENDER_WORLD;
                                            })
                                            .load(base_path.resolve_embed(&tileset_image)?),
                                        tiles: tiles
                                            .into_iter()
                                            .map(|tile| LdtkTile {
                                                id: tile.t,
                                                grid_position_px: uvec2(tile.px[0], top_px - tile.px[1]),
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
