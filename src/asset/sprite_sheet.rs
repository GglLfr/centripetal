use std::io;

use async_channel::Sender;
use bevy::{
    asset::{AssetLoader, AsyncReadExt as _, LoadContext, LoadDirectError, ParseAssetPathError, io::Reader},
    platform::collections::HashMap,
    prelude::*,
};
use derive_more::{Display, Error, From};
use serde::Deserialize;

use crate::asset::SpriteError;

#[derive(Debug, Clone, Asset, TypePath)]
pub struct SpriteSheet {
    pub page: Handle<Image>,
    pub sprite: TextureAtlas,
    pub frames: Vec<AnimFrame>,
    pub animations: HashMap<String, Anim>,
}

#[derive(Debug, Copy, Clone)]
pub struct AnimFrame {
    pub uv_rect: URect,
    pub rect: Rect,
    pub duration: u32,
}

#[derive(Debug, Copy, Clone)]
pub struct Anim {
    pub from: usize,
    pub to: usize,
    pub direction: AnimDirection,
}

#[derive(Debug, Copy, Clone)]
pub enum AnimDirection {
    Forward,
}

#[derive(Debug, Error, From, Display)]
pub enum SpriteSheetError {
    Image(LoadDirectError),
    Packing(SpriteError),
    Path(ParseAssetPathError),
    Json(serde_json::Error),
    Io(io::Error),
    #[display("The channel to the main world for packing sprites was closed")]
    Closed,
}

#[derive(Debug, Clone)]
pub struct SpriteSheetLoader(pub Sender<(Image, Sender<Result<(Handle<Image>, TextureAtlas), SpriteError>>)>);
impl AssetLoader for SpriteSheetLoader {
    type Asset = SpriteSheet;
    type Settings = ();
    type Error = SpriteSheetError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _: &Self::Settings,
        load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        #[derive(Deserialize)]
        struct SRoot {
            frames: Vec<SFrame>,
            meta: SMeta,
        }

        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct SFrame {
            filename: String,
            frame: SRect,
            sprite_source_size: SRect,
            source_size: SSize,
            duration: u32,
        }

        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct SMeta {
            image: String,
            frame_tags: Vec<STag>,
        }

        #[derive(Deserialize)]
        struct STag {
            name: String,
            from: usize,
            to: usize,
            direction: SDirection,
        }

        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        enum SDirection {
            Forward,
        }

        #[derive(Deserialize)]
        struct SRect {
            x: u32,
            y: u32,
            #[serde(flatten)]
            size: SSize,
        }

        #[derive(Deserialize)]
        struct SSize {
            w: u32,
            h: u32,
        }

        let mut file = String::new();
        reader.read_to_string(&mut file).await?;

        let mut root: SRoot = serde_json::from_str(&file)?;

        let image_path = load_context.asset_path().resolve_embed(&root.meta.image)?;
        let image = load_context.loader().immediate().load(image_path).await?.take();

        let (sender, receiver) = async_channel::bounded(1);
        self.0.send((image, sender)).await.map_err(|_| SpriteSheetError::Closed)?;
        let (page, sprite) = receiver.recv().await.map_err(|_| SpriteSheetError::Closed)??;

        root.frames
            .sort_unstable_by_key(|frame| u32::from_str_radix(&frame.filename, 10).unwrap_or(0));

        let frames = root
            .frames
            .into_iter()
            .map(|frame| AnimFrame {
                uv_rect: URect {
                    min: UVec2::new(frame.frame.x, frame.frame.y),
                    max: UVec2::new(frame.frame.x + frame.frame.size.w, frame.frame.y + frame.frame.size.h),
                },
                rect: Rect {
                    min: Vec2::new(
                        frame.sprite_source_size.x as f32 - frame.source_size.w as f32 / 2.,
                        frame.sprite_source_size.y as f32 - frame.source_size.h as f32 / 2.,
                    ),
                    max: Vec2::new(
                        (frame.sprite_source_size.x + frame.sprite_source_size.size.w) as f32 -
                            frame.source_size.w as f32 / 2.,
                        (frame.sprite_source_size.y + frame.sprite_source_size.size.h) as f32 -
                            frame.source_size.h as f32 / 2.,
                    ),
                },
                duration: frame.duration,
            })
            .collect();

        let animations = root
            .meta
            .frame_tags
            .into_iter()
            .map(|tag| {
                (tag.name, Anim {
                    from: tag.from,
                    to: tag.to,
                    direction: match tag.direction {
                        SDirection::Forward => AnimDirection::Forward,
                    },
                })
            })
            .collect();

        Ok(SpriteSheet {
            page,
            sprite,
            frames,
            animations,
        })
    }

    fn extensions(&self) -> &[&str] {
        &["json"]
    }
}
