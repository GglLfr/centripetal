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
    pub event_tags: HashMap<String, AnimationIndices>,
}

#[derive(Reflect, Debug)]
pub struct AnimationFrame {
    pub region: Handle<AtlasRegion>,
    pub offset: Vec2,
    pub duration: Duration,
    pub slices: HashMap<String, Rect>,
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
            frames: BTreeMap<usize, FrameRepr>,
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
            slices: Vec<SliceRepr>,
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

        #[derive(Deserialize)]
        struct SliceRepr {
            name: String,
            keys: Vec<SliceKey>,
        }

        #[derive(Deserialize)]
        struct SliceKey {
            frame: usize,
            bounds: RectRepr,
        }

        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;

        let repr = serde_json::from_slice::<Repr>(&bytes)?;
        let region_path = load_context.asset_path().resolve_embed(&repr.meta.image)?;
        let region = load_context.loader().immediate().load::<AtlasRegion>(region_path).await?;
        let region_ref = region.get();

        let mut frame_tags = HashMap::new();
        let mut event_tags = HashMap::new();
        for tag in repr.meta.frameTags {
            match tag.name.split_at_checked(2) {
                None => Err(format!("Invalid tag name {}", tag.name))?,
                Some((ident, name)) => {
                    let tag = AnimationIndices {
                        indices: tag.from..=tag.to,
                        direction: tag.direction,
                    };

                    match ident {
                        "f:" => &mut frame_tags,
                        "e:" => &mut event_tags,
                        unknown => Err(format!("Unknown tag category {}", &unknown[0..1]))?,
                    }
                    .insert(name.into(), tag);
                }
            }
        }

        Ok(AnimationSheet {
            frames: repr.frames.into_iter().try_map_into_default(|(i, frame)| {
                let frame_pos = uvec2(frame.x, frame.y);
                let frame_size = uvec2(frame.w, frame.h);
                let src_size = uvec2(frame.sourceSize.w, frame.sourceSize.h).as_vec2();

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
                        frame.spriteSourceSize.x as f32 + (frame.spriteSourceSize.w as f32) / 2. - src_size.x / 2.,
                        -(frame.spriteSourceSize.y as f32 + (frame.spriteSourceSize.h as f32) / 2. - src_size.y / 2.),
                    ),
                    duration: Duration::from_millis(frame.duration),
                    slices: repr
                        .meta
                        .slices
                        .iter()
                        .flat_map(|slice| {
                            slice.keys.iter().filter_map(|key| {
                                (key.frame == i).then(|| {
                                    let rect = Rect {
                                        min: vec2(key.bounds.x as f32, src_size.y - (key.bounds.y + key.bounds.h) as f32) - src_size / 2.,
                                        max: vec2((key.bounds.x + key.bounds.w) as f32, src_size.y - key.bounds.y as f32) - src_size / 2.,
                                    };

                                    (slice.name.clone(), rect)
                                })
                            })
                        })
                        .collect(),
                })
            })?,
            region: load_context.add_loaded_labeled_asset("region", region),
            frame_tags,
            event_tags,
        })
    }
}

pub(super) fn plugin(app: &mut App) {
    app.init_asset::<AnimationSheet>()
        .register_asset_reflect::<AnimationSheet>()
        .register_asset_loader(AnimationSheetLoader);
}
