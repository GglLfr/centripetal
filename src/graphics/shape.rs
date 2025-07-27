use bevy::{
    asset::weak_handle,
    core_pipeline::core_2d::CORE_2D_DEPTH_FORMAT,
    ecs::{
        entity::EntityHashMap,
        query::ROQueryItem,
        system::{RunSystemOnce, SystemParamItem, lifetimeless::SRes},
    },
    prelude::*,
    render::{
        mesh::PrimitiveTopology,
        render_phase::{PhaseItem, RenderCommandResult, TrackedRenderPass},
        render_resource::{
            AddressMode, BindGroupEntries, BindGroupLayout, BindGroupLayoutEntries, BlendState,
            CachedRenderPipelineId, ColorTargetState, ColorWrites, CompareFunction, DepthBiasState,
            DepthStencilState, FilterMode, FragmentState, FrontFace, MultisampleState,
            PipelineCache, PolygonMode, PrimitiveState, RenderPipelineDescriptor, Sampler,
            SamplerBindingType, SamplerDescriptor, ShaderStages, StencilFaceState, StencilState,
            TextureSampleType, VertexState,
            binding_types::{sampler, texture_2d},
        },
        renderer::RenderDevice,
        view::ViewTarget,
    },
};

use crate::graphics::{Fbo, FboWrappedDrawer};

pub const SHAPE_SHADER: Handle<Shader> = weak_handle!("3281325d-b853-4e27-a10f-d0c006454148");

#[derive(Debug, Clone, Resource)]
pub struct BlitPixelizedShapes {
    fbo_layout: BindGroupLayout,
    sampler: Sampler,
}

impl FromWorld for BlitPixelizedShapes {
    fn from_world(world: &mut World) -> Self {
        world.run_system_once(init_blit_pixelized_shapes).unwrap()
    }
}

fn init_blit_pixelized_shapes(device: Res<RenderDevice>) -> BlitPixelizedShapes {
    let fbo_layout = device.create_bind_group_layout(
        Some("centripetal_pixelized_shapes_layout"),
        &BindGroupLayoutEntries::sequential(
            ShaderStages::FRAGMENT,
            (
                texture_2d(TextureSampleType::Float { filterable: true }),
                sampler(SamplerBindingType::NonFiltering),
            ),
        ),
    );

    let sampler = device.create_sampler(&SamplerDescriptor {
        label: Some("centripetal_pixelized_shapes_sampler"),
        address_mode_u: AddressMode::ClampToEdge,
        address_mode_v: AddressMode::ClampToEdge,
        address_mode_w: AddressMode::ClampToEdge,
        mag_filter: FilterMode::Nearest,
        min_filter: FilterMode::Nearest,
        ..default()
    });

    BlitPixelizedShapes {
        fbo_layout,
        sampler,
    }
}

#[derive(Debug, Clone, Default, Resource, Deref, DerefMut)]
pub struct BlitPixelizedShapesPipelines(EntityHashMap<CachedRenderPipelineId>);

pub fn prepare_blit_pixelized_shape_pipelines(
    shapes: Res<BlitPixelizedShapes>,
    mut pipelines: ResMut<BlitPixelizedShapesPipelines>,
    cache: Res<PipelineCache>,
    cameras: Query<(Entity, &ViewTarget, &Msaa)>,
) {
    for (e, target, &msaa) in &cameras {
        if !pipelines.contains_key(&e) {
            pipelines.insert(
                e,
                cache.queue_render_pipeline(RenderPipelineDescriptor {
                    label: Some("centripetal_pixelized_shapes_pipeline".into()),
                    layout: vec![shapes.fbo_layout.clone()],
                    push_constant_ranges: vec![],
                    vertex: VertexState {
                        shader: SHAPE_SHADER,
                        shader_defs: vec![],
                        entry_point: "vertex_main".into(),
                        buffers: vec![],
                    },
                    primitive: PrimitiveState {
                        topology: PrimitiveTopology::TriangleList,
                        strip_index_format: None,
                        front_face: FrontFace::Ccw,
                        cull_mode: None,
                        unclipped_depth: false,
                        polygon_mode: PolygonMode::Fill,
                        conservative: false,
                    },
                    depth_stencil: Some(DepthStencilState {
                        format: CORE_2D_DEPTH_FORMAT,
                        depth_write_enabled: false,
                        depth_compare: CompareFunction::GreaterEqual,
                        stencil: StencilState {
                            front: StencilFaceState::IGNORE,
                            back: StencilFaceState::IGNORE,
                            read_mask: 0,
                            write_mask: 0,
                        },
                        bias: DepthBiasState {
                            constant: 0,
                            slope_scale: 0.0,
                            clamp: 0.0,
                        },
                    }),
                    multisample: MultisampleState {
                        count: msaa.samples(),
                        mask: !0,
                        alpha_to_coverage_enabled: false,
                    },
                    fragment: Some(FragmentState {
                        shader: SHAPE_SHADER,
                        shader_defs: vec![],
                        entry_point: "fragment_main".into(),
                        targets: vec![Some(ColorTargetState {
                            format: target.main_texture_format(),
                            blend: Some(BlendState::ALPHA_BLENDING),
                            write_mask: ColorWrites::all(),
                        })],
                    }),
                    zero_initialize_workgroup_memory: false,
                }),
            );
        }
    }
}

impl<P: PhaseItem> FboWrappedDrawer<P> for BlitPixelizedShapes {
    type Param = (
        SRes<Self>,
        SRes<RenderDevice>,
        SRes<BlitPixelizedShapesPipelines>,
        SRes<PipelineCache>,
    );
    type ViewQuery = Entity;
    type ItemQuery = ();

    fn render<'w>(
        fbo: Fbo,
        _: &P,
        view_entity: ROQueryItem<'w, Self::ViewQuery>,
        _: Option<ROQueryItem<'w, Self::ItemQuery>>,
        (shapes, device, pipelines, pipeline_cache): SystemParamItem<'w, '_, Self::Param>,
        pass: &mut TrackedRenderPass<'w>,
    ) -> RenderCommandResult {
        let Some(&id) = pipelines.get(&view_entity) else {
            return RenderCommandResult::Skip;
        };
        let Some(pipeline) = pipeline_cache.into_inner().get_render_pipeline(id) else {
            return RenderCommandResult::Skip;
        };

        pass.set_render_pipeline(pipeline);
        pass.wgpu_pass().set_bind_group(
            0,
            &device.create_bind_group(
                Some("centripetal_pixelized_shapes"),
                &shapes.fbo_layout,
                &BindGroupEntries::sequential((&fbo.texture.default_view, &shapes.sampler)),
            ),
            &[],
        );
        pass.draw(0..3, 0..1);

        RenderCommandResult::Success
    }
}
