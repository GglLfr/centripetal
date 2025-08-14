use bevy::render::render_phase::{Draw, DrawError};

use crate::prelude::*;

pub trait FboWrappedDrawer<P: PhaseItem>: 'static + Send + Sync {
    type Param: 'static + ReadOnlySystemParam;
    type ViewQuery: ReadOnlyQueryData;
    type ItemQuery: ReadOnlyQueryData;

    fn render<'w>(
        fbo: Fbo,
        item: &P,
        view: ROQueryItem<'w, Self::ViewQuery>,
        entity: Option<ROQueryItem<'w, Self::ItemQuery>>,
        param: SystemParamItem<'w, '_, Self::Param>,
        pass: &mut TrackedRenderPass<'w>,
    ) -> RenderCommandResult;
}

pub struct FboWrappedDraw<
    P: 'static + PhaseItem,
    C: 'static + RenderCommand<P, Param: ReadOnlySystemParam> + Send + Sync,
    T: FboWrappedDrawer<P>,
> {
    fetcher: FboFetcher,
    command: RenderCommandState<P, C>,
    state: SystemState<T::Param>,
    view: QueryState<T::ViewQuery>,
    entity: QueryState<T::ItemQuery>,
}

impl<
    P: 'static + PhaseItem,
    C: 'static + RenderCommand<P, Param: ReadOnlySystemParam> + Send + Sync,
    T: FboWrappedDrawer<P>,
> FboWrappedDraw<P, C, T>
{
    pub fn new(world: &mut World) -> Self {
        Self {
            fetcher: FboFetcher {
                state: SystemState::new(world),
            },
            command: RenderCommandState::new(world),
            state: SystemState::new(world),
            view: QueryState::new(world),
            entity: QueryState::new(world),
        }
    }
}

impl<
    P: 'static + PhaseItem,
    C: 'static + RenderCommand<P, Param: ReadOnlySystemParam> + Send + Sync,
    T: FboWrappedDrawer<P>,
> Draw<P> for FboWrappedDraw<P, C, T>
{
    fn prepare(&mut self, world: &World) {
        self.fetcher.state.update_archetypes(world);
        self.command.prepare(world);
        self.state.update_archetypes(world);
        self.view.update_archetypes(world);
        self.entity.update_archetypes(world);
    }

    fn draw<'w>(
        &mut self,
        world: &'w World,
        pass: &mut TrackedRenderPass<'w>,
        view: Entity,
        item: &P,
    ) -> Result<(), DrawError> {
        let fbo = self.fetcher.fetch(view, world)?;

        let device = world.resource::<RenderDevice>();
        let mut encoder = device.create_command_encoder(&default());

        {
            let mut inner_pass = TrackedRenderPass::new(
                &device,
                encoder.begin_render_pass(&RenderPassDescriptor {
                    label: Some("centripetal_fbo_wrapped_pass"),
                    color_attachments: &[Some(
                        if let Some(ref sampled_texture) = fbo.sampled_texture {
                            RenderPassColorAttachment {
                                view: &sampled_texture.default_view,
                                resolve_target: Some(&fbo.texture.default_view),
                                ops: default(),
                            }
                        } else {
                            RenderPassColorAttachment {
                                view: &fbo.texture.default_view,
                                resolve_target: None,
                                ops: default(),
                            }
                        },
                    )],
                    depth_stencil_attachment: Some(RenderPassDepthStencilAttachment {
                        view: &fbo.depth_texture.default_view,
                        depth_ops: Some(default()),
                        stencil_ops: None,
                    }),
                    timestamp_writes: None,
                    occlusion_query_set: None,
                }),
            );

            self.command.draw(world, &mut inner_pass, view, item)?;
        }

        let buffer = encoder.finish();
        world.resource::<RenderQueue>().submit([buffer]);

        let param = self.state.get_manual(world);
        let view = match self.view.get_manual(world, view) {
            Ok(view) => view,
            Err(err) => match err {
                QueryEntityError::EntityDoesNotExist(_) => {
                    return Err(DrawError::ViewEntityNotFound);
                }
                QueryEntityError::QueryDoesNotMatch(_, _)
                | QueryEntityError::AliasedMutability(_) => {
                    return Err(DrawError::InvalidViewQuery);
                }
            },
        };

        let entity = self.entity.get_manual(world, item.entity()).ok();
        match T::render(fbo, item, view, entity, param, pass) {
            RenderCommandResult::Success | RenderCommandResult::Skip => Ok(()),
            RenderCommandResult::Failure(reason) => Err(DrawError::RenderCommandFailure(reason)),
        }
    }
}

pub struct Fbo {
    pub texture: CachedTexture,
    pub sampled_texture: Option<CachedTexture>,
    pub depth_texture: CachedTexture,
    pub texture_format: TextureFormat,
}

pub struct FboFetcher {
    pub state: SystemState<(
        SRes<RenderDevice>,
        SRes<LockedTextureCache>,
        SQuery<(Read<ExtractedCamera>, Read<ExtractedView>, Read<Msaa>)>,
    )>,
}

impl FboFetcher {
    pub fn fetch(&mut self, camera_entity: Entity, world: &World) -> Result<Fbo, DrawError> {
        let (device, texture_cache, cameras) = self.state.get_manual(world);
        let (camera, view, msaa) = cameras
            .get(camera_entity)
            .map_err(|_| DrawError::InvalidViewQuery)?;

        let target_size = camera.physical_target_size.unwrap_or(UVec2::splat(2));
        let size = Extent3d {
            width: target_size.x,
            height: target_size.y,
            depth_or_array_layers: 1,
        };

        let texture_format = if view.hdr {
            ViewTarget::TEXTURE_FORMAT_HDR
        } else {
            TextureFormat::bevy_default()
        };

        let view_formats: &[TextureFormat] = match texture_format {
            TextureFormat::Bgra8Unorm => &[TextureFormat::Bgra8UnormSrgb],
            TextureFormat::Rgba8Unorm => &[TextureFormat::Rgba8UnormSrgb],
            _ => &[],
        };

        let texture = texture_cache
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(
                &device,
                TextureDescriptor {
                    label: Some("centripetal_fbo"),
                    size,
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: TextureDimension::D2,
                    format: texture_format,
                    usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
                    view_formats,
                },
            );

        let sampled_texture = if msaa.samples() > 1 {
            Some(
                texture_cache
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .get(
                        &device,
                        TextureDescriptor {
                            label: Some("centripetal_fbo_sampled"),
                            size,
                            mip_level_count: 1,
                            sample_count: msaa.samples(),
                            dimension: TextureDimension::D2,
                            format: texture_format,
                            usage: TextureUsages::RENDER_ATTACHMENT,
                            view_formats,
                        },
                    ),
            )
        } else {
            None
        };

        let depth_texture = texture_cache
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(
                &device,
                TextureDescriptor {
                    label: Some("centripetal_fbo_depth"),
                    size,
                    mip_level_count: 1,
                    sample_count: msaa.samples(),
                    dimension: TextureDimension::D2,
                    format: CORE_2D_DEPTH_FORMAT,
                    usage: TextureUsages::RENDER_ATTACHMENT,
                    view_formats: &[],
                },
            );

        Ok(Fbo {
            texture,
            sampled_texture,
            depth_texture,
            texture_format,
        })
    }
}

#[derive(Resource, Default, Deref, DerefMut)]
pub struct LockedTextureCache(pub Mutex<TextureCache>);
pub fn update_locked_texture_cache(mut cache: ResMut<LockedTextureCache>) {
    cache
        .get_mut()
        .unwrap_or_else(PoisonError::into_inner)
        .update();
}
