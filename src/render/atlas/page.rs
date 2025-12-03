use guillotiere::{SimpleAtlasAllocator, euclid::Box2D, size2};

use crate::prelude::*;

#[derive(Resource, Debug, Default)]
pub struct AtlasPages {
    pages: Vec<AtlasPage>,
}

pub struct AtlasPage {
    packer: SimpleAtlasAllocator,
    info: PageInfo,
}

impl Debug for AtlasPage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AtlasPage").field("texture", &self.info.texture).finish_non_exhaustive()
    }
}

#[derive(Reflect, Debug, Default, Clone)]
#[reflect(Debug, Default, FromWorld, Clone)]
pub struct PageInfo {
    pub texture: Handle<Image>,
    pub texture_size: UVec2,
}

#[derive(Reflect, Debug, Default, Clone, Deref, DerefMut)]
#[reflect(Debug, Default, FromWorld, Clone)]
pub struct AtlasInfo {
    #[deref]
    pub page: PageInfo,
    pub rect: URect,
}

impl AtlasInfo {
    pub fn uvs(&self) -> [Vec2; 2] {
        [
            self.rect.min.as_vec2() / self.page.texture_size.as_vec2(),
            self.rect.max.as_vec2() / self.page.texture_size.as_vec2(),
        ]
    }

    pub fn uv_corners(&self) -> [Vec2; 4] {
        let [uv0, uv1] = self.uvs();
        [vec2(uv0.x, uv1.y), vec2(uv1.x, uv1.y), vec2(uv1.x, uv0.y), vec2(uv0.x, uv0.y)]
    }
}

pub type AtlasInfoSender = async_channel::Sender<AtlasInfo>;

#[derive(Debug, Clone)]
pub struct AtlasRequester(async_channel::Sender<(Image, AtlasInfoSender)>);
impl FromWorld for AtlasRequester {
    fn from_world(world: &mut World) -> Self {
        world.resource::<AtlasRequesters>().new_sender()
    }
}

impl AtlasRequester {
    pub async fn request(&self, image: Image) -> Result<AtlasInfo> {
        let (sender, receiver) = async_channel::bounded(1);
        self.0.send((image, sender)).await?;
        Ok(receiver.recv().await?)
    }
}

#[derive(Resource, Debug)]
pub struct AtlasRequesters {
    sender: async_channel::Sender<(Image, AtlasInfoSender)>,
    receiver: async_channel::Receiver<(Image, AtlasInfoSender)>,
}

impl AtlasRequesters {
    pub fn new_sender(&self) -> AtlasRequester {
        AtlasRequester(self.sender.clone())
    }
}

#[derive(Resource, Debug, Default)]
struct AtlasRequests(Vec<(AssetId<Image>, Vec<u8>, URect)>);

fn handle_atlas_requests(
    mut atlas: ResMut<AtlasPages>,
    mut requests: ResMut<AtlasRequests>,
    requesters: Res<AtlasRequesters>,
    device: Res<RenderDevice>,
    mut images: ResMut<Assets<Image>>,
) -> Result {
    let max_size = device.limits().max_texture_dimension_2d.min(8192);
    'request: while let Ok((image, mut payload_sender)) = requesters.receiver.try_recv() {
        let size = image.size();
        if size.x > max_size || size.y > max_size {
            Err(format!("Sprite of size `{size}` exceeds `{max_size}`"))?
        }

        let (Some(mut data), TextureFormat::Rgba8UnormSrgb) = (image.data, image.texture_descriptor.format) else {
            Err("Image must have existing Rgba8UnormSrgb data")?
        };

        let mut try_pack = |page: &mut AtlasPage, data: Vec<u8>, payload_sender: async_channel::Sender<AtlasInfo>| {
            if let Some(Box2D { min, max }) = page.packer.allocate(size2(size.x as i32 + 2, size.y as i32 + 2)) {
                let rect = URect {
                    min: uvec2(min.x as u32 + 1, min.y as u32 + 1),
                    max: uvec2(max.x as u32 - 1, max.y as u32 - 1),
                };

                let page_info = page.info.clone();
                requests.0.push((page.info.texture.id(), data, rect));
                AsyncComputeTaskPool::get()
                    .spawn(async move { _ = payload_sender.send(AtlasInfo { page: page_info, rect }).await })
                    .detach();
                Ok(())
            } else {
                Err((data, payload_sender))
            }
        };

        for page in &mut atlas.pages {
            match try_pack(page, data, payload_sender) {
                Ok(()) => continue 'request,
                Err((failed_data, failed_sender)) => {
                    data = failed_data;
                    payload_sender = failed_sender;
                }
            }
        }

        let mut new_page = AtlasPage {
            packer: SimpleAtlasAllocator::new(size2(max_size as i32, max_size as i32)),
            info: PageInfo {
                texture: images.add(Image::new_fill(
                    Extent3d {
                        width: max_size,
                        height: max_size,
                        depth_or_array_layers: 1,
                    },
                    TextureDimension::D2,
                    &[0, 0, 0, 0],
                    TextureFormat::Rgba8UnormSrgb,
                    RenderAssetUsages::RENDER_WORLD,
                )),
                texture_size: UVec2::splat(max_size),
            },
        };

        try_pack(&mut new_page, data, payload_sender).expect("Atlas page should be big enough");
        atlas.pages.push(new_page);
    }

    Ok(())
}

#[derive(Resource, Debug, Default)]
struct ExtractedAtlasRequests(Vec<(AssetId<Image>, Vec<u8>, URect)>);

fn extract_atlas_requests(mut main_world: ResMut<MainWorld>, mut requests: ResMut<ExtractedAtlasRequests>) {
    mem::swap(&mut main_world.resource_mut::<AtlasRequests>().0, &mut requests.0);
}

fn prepare_atlas_requests(mut requests: ResMut<ExtractedAtlasRequests>, gpu_images: Res<RenderAssets<GpuImage>>, queue: Res<RenderQueue>) {
    requests.0.retain(|&(page, ref data, rect)| {
        let Some(gpu_image) = gpu_images.get(page) else { return true };
        queue.write_texture(
            TexelCopyTextureInfo {
                texture: &gpu_image.texture,
                mip_level: 0,
                origin: Origin3d {
                    x: rect.min.x,
                    y: rect.min.y,
                    z: 0,
                },
                aspect: TextureAspect::All,
            },
            &data,
            TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(rect.size().x * 4),
                rows_per_image: None,
            },
            Extent3d {
                width: rect.size().x,
                height: rect.size().y,
                depth_or_array_layers: 1,
            },
        );
        false
    });
}

pub(super) fn plugin(app: &mut App) {
    let (sender, receiver) = async_channel::bounded(8);
    app.init_resource::<AtlasPages>()
        .init_resource::<AtlasRequests>()
        .insert_resource(AtlasRequesters { sender, receiver })
        .add_systems(Update, handle_atlas_requests);

    if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
        render_app
            .init_resource::<ExtractedAtlasRequests>()
            .add_systems(ExtractSchedule, extract_atlas_requests)
            .add_systems(Render, prepare_atlas_requests.in_set(RenderSystems::PrepareResources));
    }
}
