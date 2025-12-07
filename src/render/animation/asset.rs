use crate::{
    prelude::*,
    render::atlas::{AtlasInfo, AtlasRegion},
    util::IteratorExt,
};

#[derive(Reflect, Asset, Debug)]
pub struct AnimationSheet {
    pub region: Handle<AtlasRegion>,
    pub frames: Vec<AnimationFrame>,
    pub frame_tags: HashMap<String, AnimationIndices>,
}

#[derive(Reflect, Debug)]
pub struct AnimationFrame {
    pub region: Handle<AtlasRegion>,
    pub offset: Vec2,
    pub duration: Duration,
}

#[derive(Reflect, Debug)]
pub struct AnimationIndices {
    pub indices: RangeInclusive<usize>,
    pub direction: AnimationDirection,
}

#[derive(Reflect, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AnimationDirection {
    Forward,
    Reverse,
}

pub struct AnimationSheetLoader;
impl AssetLoader for AnimationSheetLoader {
    type Asset = AnimationSheet;
    type Settings = ();
    type Error = BevyError;

    async fn load(&self, reader: &mut dyn Reader, _: &Self::Settings, load_context: &mut LoadContext<'_>) -> Result<Self::Asset, Self::Error> {
        #[derive(Deserialize)]
        struct Repr {
            frames: BTreeMap<u32, FrameRepr>,
            meta: MetaRepr,
        }

        #[derive(Deserialize, Deref)]
        #[expect(non_snake_case, reason = "Aseprite spritesheet naming scheme")]
        struct FrameRepr {
            #[deref]
            frame: RectRepr,
            spriteSourceSize: RectRepr,
            sourceSize: SizeRepr,
            duration: u64,
        }

        #[derive(Deserialize)]
        #[expect(non_snake_case, reason = "Aseprite spritesheet naming scheme")]
        struct MetaRepr {
            image: String,
            frameTags: Vec<FrameTagRepr>,
        }

        #[derive(Deserialize)]
        struct FrameTagRepr {
            name: String,
            from: usize,
            to: usize,
            direction: AnimationDirection,
        }

        #[derive(Deserialize, Deref)]
        struct RectRepr {
            x: u32,
            y: u32,
            #[serde(flatten)]
            #[deref]
            size: SizeRepr,
        }

        #[derive(Deserialize)]
        struct SizeRepr {
            w: u32,
            h: u32,
        }

        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;

        let repr = serde_json::from_slice::<Repr>(&bytes)?;
        let region_path = load_context.asset_path().resolve_embed(&repr.meta.image)?;
        let region = load_context.loader().immediate().load::<AtlasRegion>(region_path).await?;
        let region_ref = region.get();

        Ok(AnimationSheet {
            frames: repr.frames.into_iter().try_map_into_default(|(i, frame)| {
                let frame_pos = uvec2(frame.x, frame.y);
                let frame_size = uvec2(frame.w, frame.h);

                Ok::<_, BevyError>(AnimationFrame {
                    region: load_context.add_labeled_asset(format!("frame#{i}"), AtlasRegion {
                        info: AtlasInfo {
                            page: region_ref.page.clone(),
                            rect: URect {
                                min: region_ref.rect.min + frame_pos,
                                max: region_ref.rect.min + frame_pos + frame_size,
                            },
                        },
                    }),
                    offset: vec2(
                        frame.spriteSourceSize.x as f32 + (frame.spriteSourceSize.w as f32) / 2. - frame.sourceSize.w as f32 / 2.,
                        -(frame.spriteSourceSize.y as f32 + (frame.spriteSourceSize.h as f32) / 2. - frame.sourceSize.h as f32 / 2.),
                    ),
                    duration: Duration::from_millis(frame.duration),
                })
            })?,
            region: load_context.add_loaded_labeled_asset("region", region),
            frame_tags: repr
                .meta
                .frameTags
                .into_iter()
                .map(|tag| {
                    (tag.name, AnimationIndices {
                        indices: tag.from..=tag.to,
                        direction: tag.direction,
                    })
                })
                .collect(),
        })
    }
}

pub(super) fn plugin(app: &mut App) {
    app.init_asset::<AnimationSheet>()
        .register_asset_reflect::<AnimationSheet>()
        .register_asset_loader(AnimationSheetLoader);
}
