use async_channel::Sender;
use bevy::image::{CompressedImageFormats, ImageLoader, ImageLoaderError, ImageLoaderSettings};
use blocking::unblock;

use crate::{graphics::SpriteError, prelude::*};

#[derive(Debug, Clone, Asset, TypePath)]
pub struct SpriteSection {
    pub page: Handle<Image>,
    pub sprite: TextureAtlas,
    pub rect: Option<Rect>,
    pub center_anchor: Anchor,
    pub size: Vec2,
}

impl SpriteSection {
    pub fn sprite(&self) -> Sprite {
        Sprite {
            image: self.page.clone(),
            texture_atlas: Some(self.sprite.clone()),
            color: Color::WHITE,
            flip_x: false,
            flip_y: false,
            custom_size: Some(self.size),
            rect: self.rect,
            anchor: self.center_anchor,
            image_mode: SpriteImageMode::Auto,
        }
    }

    pub fn sprite_with(
        &self,
        color: impl Into<Color>,
        size: impl Into<Option<Vec2>>,
        local_anchor: Anchor,
    ) -> Sprite {
        let size = size.into().unwrap_or(self.size);
        Sprite {
            image: self.page.clone(),
            texture_atlas: Some(self.sprite.clone()),
            color: color.into(),
            flip_x: false,
            flip_y: false,
            custom_size: Some(size),
            rect: self.rect,
            anchor: Anchor::Custom(
                self.center_anchor.as_vec()
                    + local_anchor.as_vec() * size
                        / self.rect.map(|rect| rect.size()).unwrap_or(size),
            ),
            image_mode: SpriteImageMode::Auto,
        }
    }
}

#[derive(Debug, Error, From, Display)]
pub enum SpriteSectionError {
    Image(ImageLoaderError),
    Packing(SpriteError),
    #[display("The channel to the main world for packing sprites was closed")]
    Closed,
}

#[derive(Debug, Clone)]
pub struct SpriteSectionLoader(
    pub  Sender<(
        Image,
        Sender<Result<(Handle<Image>, TextureAtlas), SpriteError>>,
    )>,
);
impl AssetLoader for SpriteSectionLoader {
    type Asset = SpriteSection;
    type Settings = ();
    type Error = SpriteSectionError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _: &Self::Settings,
        load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let image = ImageLoader::new(CompressedImageFormats::empty())
            .load(reader, &ImageLoaderSettings::default(), load_context)
            .await?;

        let (sender, receiver) = async_channel::bounded(1);
        let size = image.size().as_vec2();
        self.0
            .send((image, sender))
            .await
            .map_err(|_| SpriteSectionError::Closed)?;
        let (page, sprite) = receiver
            .recv()
            .await
            .map_err(|_| SpriteSectionError::Closed)??;

        Ok(SpriteSection {
            page,
            sprite,
            rect: None,
            center_anchor: Anchor::Center,
            size,
        })
    }

    fn extensions(&self) -> &[&str] {
        ImageLoader::SUPPORTED_FILE_EXTENSIONS
    }
}

#[derive(Debug, Clone, Asset, TypePath)]
pub struct SpriteSheet {
    pub page: Handle<Image>,
    pub sprite: TextureAtlas,
    pub frames: Vec<Handle<SpriteSection>>,
    pub durations: Vec<Duration>,
    pub tags: HashMap<String, Range<usize>>,
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
}

#[derive(Debug, Clone)]
pub struct SpriteSheetLoader(
    pub  Sender<(
        Image,
        Sender<Result<(Handle<Image>, TextureAtlas), SpriteError>>,
    )>,
);
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
        let image = load_context
            .loader()
            .immediate()
            .load(image_path)
            .await?
            .take();

        let (sender, receiver) = async_channel::bounded(1);
        self.0
            .send((image, sender))
            .await
            .map_err(|_| SpriteSheetError::Closed)?;
        let (page, sprite) = receiver
            .recv()
            .await
            .map_err(|_| SpriteSheetError::Closed)??;

        frames.sort_unstable_by_key(|frame| frame.filename.parse::<u32>().unwrap_or(0));
        let durations = frames
            .iter()
            .map(|frame| Duration::from_millis(frame.duration as u64))
            .collect();

        let frames = frames
            .iter()
            .enumerate()
            .map(|(i, frame)| {
                let source_size = uvec2(frame.source_size.w, frame.source_size.h).as_vec2();
                let source_center = source_size / 2.;

                let frame_location = uvec2(frame.frame.x, frame.frame.y).as_vec2();
                let frame_size = uvec2(frame.frame.w, frame.frame.h).as_vec2();

                let sprite_location =
                    uvec2(frame.sprite_source_size.x, frame.sprite_source_size.y).as_vec2();
                let sprite_size =
                    uvec2(frame.sprite_source_size.w, frame.sprite_source_size.h).as_vec2();
                let sprite_center = sprite_location + sprite_size / 2.;

                load_context.add_labeled_asset(
                    format!("frame-{i}"),
                    SpriteSection {
                        page: page.clone_weak(),
                        sprite: TextureAtlas {
                            layout: sprite.layout.clone_weak(),
                            index: sprite.index,
                        },
                        rect: Some(Rect {
                            min: frame_location,
                            max: frame_location + frame_size,
                        }),
                        center_anchor: Anchor::Custom(
                            (source_center - sprite_center) / sprite_size * vec2(1., -1.),
                        ),
                        size: sprite_size,
                    },
                )
            })
            .collect();

        let tags = meta
            .frame_tags
            .into_iter()
            .map(|tag| (tag.name, tag.from..tag.to))
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
