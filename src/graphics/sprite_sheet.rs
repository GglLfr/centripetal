use std::{io, ops::Range, time::Duration};

use async_channel::Sender;
use bevy::{
    asset::{AssetLoader, AsyncReadExt as _, LoadContext, LoadDirectError, ParseAssetPathError, io::Reader},
    platform::collections::HashMap,
    prelude::*,
    sprite::Anchor,
};
use blocking::unblock;
use derive_more::{Display, Error, From};
use serde::Deserialize;

use crate::graphics::SpriteError;

#[derive(Debug, Clone, Asset, TypePath)]
pub struct SpriteSheet {
    pub page: Handle<Image>,
    pub sprite: TextureAtlas,
    pub frames: Vec<Sprite>,
    pub durations: Vec<Duration>,
    pub tags: HashMap<String, (Range<usize>, Direction)>,
}

#[derive(Debug, Copy, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Direction {
    #[default]
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

#[derive(Deserialize)]
struct Wh {
    w: u32,
    h: u32,
}

#[derive(Deserialize)]
struct Xywh {
    x: u32,
    y: u32,
    w: u32,
    h: u32,
}

#[derive(Deserialize)]
struct Root {
    frames: Vec<Frame>,
    meta: Meta,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Frame {
    /// This is only used for sorting, and *must* be saved as `{frame}` from Aseprite.
    filename: String,
    /// Localized UV coordinates; where the frame is located within the sprite.
    frame: Xywh,
    /// Trim offset; frame offset from the source's top-left, and how much space it occupies.
    sprite_source_size: Xywh,
    /// The size of the source frame itself, equivalent to the canvas size as seen in Aseprite.
    source_size: Wh,
    /// The duration of the frame before moving on to the next one, in milliseconds.
    duration: u32,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Meta {
    /// Path to the sprite.
    image: String,
    frame_tags: Vec<Tag>,
}

#[derive(Deserialize)]
struct Tag {
    name: String,
    from: usize,
    to: usize,
    direction: Direction,
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
        let mut file = String::new();
        reader.read_to_string(&mut file).await?;

        let Root { mut frames, meta } = unblock(move || serde_json::from_str(&file)).await?;
        let image_path = load_context.asset_path().resolve_embed(&meta.image)?;
        let image = load_context.loader().immediate().load(image_path).await?.take();

        let (sender, receiver) = async_channel::bounded(1);
        self.0.send((image, sender)).await.map_err(|_| SpriteSheetError::Closed)?;
        let (page, sprite) = receiver.recv().await.map_err(|_| SpriteSheetError::Closed)??;

        frames.sort_unstable_by_key(|frame| frame.filename.parse::<u32>().unwrap_or(0));
        let durations = frames
            .iter()
            .map(|frame| Duration::from_millis(frame.duration as u64))
            .collect();

        let frames = frames
            .iter()
            .map(|frame| {
                let source_size = uvec2(frame.source_size.w, frame.source_size.h).as_vec2();
                let source_center = source_size / 2.;

                let frame_location = uvec2(frame.frame.x, frame.frame.y).as_vec2();
                let frame_size = uvec2(frame.frame.w, frame.frame.h).as_vec2();

                let sprite_location = uvec2(frame.sprite_source_size.x, frame.sprite_source_size.y).as_vec2();
                let sprite_size = uvec2(frame.sprite_source_size.w, frame.sprite_source_size.h).as_vec2();
                let sprite_center = sprite_location + sprite_size / 2.;

                Sprite {
                    image: page.clone_weak(),
                    texture_atlas: Some(TextureAtlas {
                        layout: sprite.layout.clone_weak(),
                        index: sprite.index,
                    }),
                    color: Color::WHITE,
                    flip_x: false,
                    flip_y: false,
                    custom_size: None,
                    rect: Some(Rect {
                        min: frame_location,
                        max: frame_location + frame_size,
                    }),
                    anchor: Anchor::Custom((source_center - sprite_center) / sprite_size),
                    image_mode: SpriteImageMode::Auto,
                }
            })
            .collect();

        let tags = meta
            .frame_tags
            .into_iter()
            .map(|tag| (tag.name, (tag.from..tag.to, tag.direction)))
            .collect();

        Ok(SpriteSheet {
            page,
            sprite,
            frames,
            durations,
            tags,
        })
    }

    fn extensions(&self) -> &[&str] {
        &["json"]
    }
}
