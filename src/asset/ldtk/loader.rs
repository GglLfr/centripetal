use std::io;

use bevy::{
    asset::{AssetLoader, AsyncReadExt, LoadContext, ParseAssetPathError, io::Reader, uuid::Uuid},
    platform::collections::HashMap,
    prelude::*,
};
use derive_more::{Display, Error, From};
use serde::{
    Deserialize, Deserializer,
    de::{self, Visitor},
};

use crate::asset::ldtk::{Ldtk, LdtkLevel};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LdtkRoot<'a> {
    iid: Uuid,
    #[serde(deserialize_with = "de_color")]
    bg_color: Srgba,
    #[serde(borrow)]
    levels: Vec<LevelPaths<'a>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Definitions {}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LevelPaths<'a> {
    iid: Uuid,
    external_rel_path: &'a str,
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

        let root: LdtkRoot = serde_json::from_str(&file)?;
        let base_path = load_context.asset_path().clone();

        Ok(Ldtk {
            iid: root.iid,
            bg_color: root.bg_color,
            levels: root.levels.into_iter().try_fold(HashMap::new(), |mut out, path| {
                let level_path = base_path.resolve_embed(path.external_rel_path)?;

                out.insert(path.iid, load_context.load(level_path));
                Ok::<_, Self::Error>(out)
            })?,
        })
    }

    fn extensions(&self) -> &[&str] {
        &["ldtk"]
    }
}

#[derive(Debug, Deserialize)]
struct Level {
    iid: Uuid,
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

        let level: Level = serde_json::from_str(&file)?;

        Ok(LdtkLevel { iid: level.iid })
    }

    fn extensions(&self) -> &[&str] {
        &["ldtkl"]
    }
}
