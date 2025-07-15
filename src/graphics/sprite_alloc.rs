use async_channel::{Receiver, Sender};
use bevy::{
    asset::RenderAssetUsages,
    ecs::system::SystemState,
    image::{ImageSampler, TextureFormatPixelInfo},
    prelude::*,
    render::{
        MainWorld,
        render_asset::RenderAssets,
        render_resource::{
            Extent3d, Origin3d, TexelCopyBufferLayout, TexelCopyTextureInfo, TextureAspect, TextureDescriptor,
            TextureDimension, TextureFormat, TextureUsages, TextureViewDescriptor, TextureViewDimension,
        },
        renderer::{RenderDevice, RenderQueue},
        texture::GpuImage,
    },
    tasks::ComputeTaskPool,
};
use derive_more::{Display, Error};
use guillotiere::{SimpleAtlasAllocator, euclid::Size2D};

#[derive(Debug, Resource)]
pub struct SpriteAllocator {
    pages: Vec<Page>,
    pending: Vec<(Image, Handle<Image>, URect)>,
}

impl FromWorld for SpriteAllocator {
    fn from_world(world: &mut World) -> Self {
        let (device, mut images, mut layouts) =
            SystemState::<(Res<RenderDevice>, ResMut<Assets<Image>>, ResMut<Assets<TextureAtlasLayout>>)>::new(world)
                .get_mut(world);

        Self {
            pages: vec![Page::new(
                UVec2::splat(device.limits().max_texture_dimension_2d.min(2048)),
                &mut images,
                &mut layouts,
            )],
            pending: vec![],
        }
    }
}

#[derive(Debug, Display, Error)]
pub enum SpriteError {
    #[error(ignore)]
    #[display("Requested sprite's size is too big ({requested_size} > {max_size})")]
    SpriteTooLarge { requested_size: UVec2, max_size: UVec2 },
    #[error(ignore)]
    #[display("The associated texture atlas layout in a page has been erroneously removed somewhere else")]
    NonexistentLayout { page: AssetId<Image> },
}

impl SpriteAllocator {
    pub fn pack(
        &mut self,
        image: Image,
        images: &mut Assets<Image>,
        layouts: &mut Assets<TextureAtlasLayout>,
    ) -> Result<(Handle<Image>, TextureAtlas), SpriteError> {
        for page in &mut self.pages {
            let layout = layouts
                .get_mut(&page.layout)
                .ok_or(SpriteError::NonexistentLayout { page: page.image.id() })?;

            if let Some(rect) = page.alloc.allocate(Size2D::new(image.width() + 2, image.height() + 2).cast()) {
                let rect = URect {
                    min: IVec2::new(rect.min.x + 1, rect.min.y + 1).as_uvec2(),
                    max: IVec2::new(rect.max.x - 1, rect.max.y - 1).as_uvec2(),
                };

                let index = layout.add_texture(rect);
                self.pending.push((image, page.image.clone_weak(), rect));

                return Ok((page.image.clone_weak(), TextureAtlas {
                    layout: page.layout.clone_weak(),
                    index,
                }))
            }
        }

        let max_size = UVec2::splat(self.pages[0].alloc.size().width as u32);
        let mut page = Page::new(max_size, images, layouts);

        let layout = layouts
            .get_mut(&page.layout)
            .ok_or(SpriteError::NonexistentLayout { page: page.image.id() })?;

        match page.alloc.allocate(Size2D::new(image.width() + 2, image.height() + 2).cast()) {
            Some(rect) => {
                let rect = URect {
                    min: IVec2::new(rect.min.x + 1, rect.min.y + 1).as_uvec2(),
                    max: IVec2::new(rect.max.x - 1, rect.max.y - 1).as_uvec2(),
                };

                let index = layout.add_texture(rect);
                self.pending.push((image, page.image.clone_weak(), rect));

                let result = (page.image.clone_weak(), TextureAtlas {
                    layout: page.layout.clone_weak(),
                    index,
                });

                self.pages.push(page);
                Ok(result)
            }
            None => Err(SpriteError::SpriteTooLarge {
                requested_size: image.size(),
                max_size,
            }),
        }
    }
}

pub struct Page {
    alloc: SimpleAtlasAllocator,
    image: Handle<Image>,
    layout: Handle<TextureAtlasLayout>,
}

impl Page {
    fn new(size: UVec2, images: &mut Assets<Image>, layouts: &mut Assets<TextureAtlasLayout>) -> Self {
        let alloc = SimpleAtlasAllocator::new(Size2D::new(size.x, size.y).cast());
        let image = images.add(Image {
            data: Some(vec![
                0;
                size.x as usize *
                    size.y as usize *
                    TextureFormat::Rgba8UnormSrgb.pixel_size()
            ]),
            texture_descriptor: TextureDescriptor {
                label: Some("centripetal_sprite_page"),
                size: Extent3d {
                    width: size.x,
                    height: size.y,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: TextureDimension::D2,
                format: TextureFormat::Rgba8UnormSrgb,
                usage: TextureUsages::COPY_DST | TextureUsages::TEXTURE_BINDING,
                view_formats: &[TextureFormat::Rgba8UnormSrgb],
            },
            sampler: ImageSampler::Default,
            texture_view_descriptor: Some(TextureViewDescriptor {
                label: Some("centripetal_sprite_page_view"),
                format: Some(TextureFormat::Rgba8UnormSrgb),
                dimension: Some(TextureViewDimension::D2),
                usage: Some(TextureUsages::COPY_DST | TextureUsages::TEXTURE_BINDING),
                aspect: TextureAspect::All,
                base_mip_level: 0,
                mip_level_count: None,
                base_array_layer: 0,
                array_layer_count: None,
            }),
            asset_usage: RenderAssetUsages::RENDER_WORLD,
        });
        let layout = layouts.add(TextureAtlasLayout { size, textures: vec![] });

        Self { alloc, image, layout }
    }
}

impl std::fmt::Debug for Page {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Page").field("layout", &self.layout).finish()
    }
}

pub fn pack_incoming_sprites(
    receiver: Receiver<(Image, Sender<Result<(Handle<Image>, TextureAtlas), SpriteError>>)>,
) -> impl System<In = (), Out = ()> {
    IntoSystem::into_system(
        move |mut sprites: ResMut<SpriteAllocator>,
              mut images: ResMut<Assets<Image>>,
              mut layouts: ResMut<Assets<TextureAtlasLayout>>| {
            ComputeTaskPool::get().scope(|scope| {
                while let Ok((image, sender)) = receiver.try_recv() {
                    let result = sprites.pack(image, &mut images, &mut layouts);
                    scope.spawn(async move {
                        _ = sender.send(result).await;
                    });
                }
            });
        },
    )
}

#[derive(Debug, Default, Resource)]
pub struct PendingSprites(Vec<(Image, Handle<Image>, URect)>);

pub fn extract_pending_sprites(mut world: ResMut<MainWorld>, mut pending: ResMut<PendingSprites>) {
    pending.0.append(&mut world.resource_mut::<SpriteAllocator>().pending);
}

pub fn prepare_pending_sprites(
    mut pending: ResMut<PendingSprites>,
    gpu_images: Res<RenderAssets<GpuImage>>,
    queue: Res<RenderQueue>,
) {
    pending.0.retain(|(image, page, rect)| {
        let Some(gpu_page) = gpu_images.get(page.id()) else { return true };
        let Some(data) = image.data.as_ref() else { return true };

        queue.write_texture(
            TexelCopyTextureInfo {
                texture: &gpu_page.texture,
                mip_level: 0,
                origin: Origin3d {
                    x: rect.min.x,
                    y: rect.min.y,
                    z: 0,
                },
                aspect: TextureAspect::All,
            },
            data,
            TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(rect.size().x * TextureFormat::Rgba8UnormSrgb.pixel_size() as u32),
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
