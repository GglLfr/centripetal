use bevy::{
    asset::weak_handle,
    core_pipeline::core_2d::CORE_2D_DEPTH_FORMAT,
    ecs::{
        query::ROQueryItem,
        system::{
            RunSystemOnce, SystemParamItem,
            lifetimeless::{Read, SRes},
        },
    },
    prelude::*,
    render::{
        mesh::PrimitiveTopology,
        render_phase::{PhaseItem, RenderCommandResult, TrackedRenderPass},
        render_resource::{
            BindGroupEntries, BindGroupLayout, BindGroupLayoutEntries, BlendState,
            CachedRenderPipelineId, ColorTargetState, ColorWrites, CompareFunction, DepthBiasState,
            DepthStencilState, FragmentState, FrontFace, MultisampleState, PipelineCache,
            PolygonMode, PrimitiveState, RenderPipelineDescriptor, ShaderStages, ShaderType,
            StencilFaceState, StencilState, TextureFormat, TextureSampleType, UniformBuffer,
            VertexState,
            binding_types::{texture_2d, uniform_buffer},
        },
        renderer::{RenderDevice, RenderQueue},
        view::{ExtractedView, ViewTarget},
    },
};

use crate::graphics::{Fbo, FboWrappedDrawer};

pub const SHAPE_SHADER: Handle<Shader> = weak_handle!("3281325d-b853-4e27-a10f-d0c006454148");

#[derive(Debug, Clone, Resource)]
pub struct BlitPixelizedShapes {
    fbo_layout: BindGroupLayout,
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
            ShaderStages::VERTEX_FRAGMENT,
            (
                texture_2d(TextureSampleType::Float { filterable: false }),
                uniform_buffer::<BlitPixelizedShapesBufferValue>(false),
            ),
        ),
    );

    BlitPixelizedShapes { fbo_layout }
}

#[derive(Debug, Copy, Clone, Component, PartialEq, Eq)]
pub struct BlitPixelizedShapesPipeline {
    pub format: TextureFormat,
    pub msaa: Msaa,
    pub id: CachedRenderPipelineId,
}

#[derive(Component, Deref, DerefMut)]
pub struct BlitPixelizedShapesBuffer(pub UniformBuffer<BlitPixelizedShapesBufferValue>);

#[derive(Debug, Copy, Clone, ShaderType)]
pub struct BlitPixelizedShapesBufferValue {
    pub bottom_left: Vec2,
    pub top_right: Vec2,
}

pub fn prepare_blit_pixelized_shape_buffers(
    mut commands: Commands,
    device: Res<RenderDevice>,
    queue: Res<RenderQueue>,
    mut cameras: Query<(
        Entity,
        &ExtractedView,
        Option<&mut BlitPixelizedShapesBuffer>,
    )>,
) {
    for (e, view, buffer) in &mut cameras {
        let ndc_to_world = view.world_from_view.compute_matrix() * view.clip_from_view.inverse();
        let value = BlitPixelizedShapesBufferValue {
            bottom_left: ndc_to_world.project_point3(vec3(-1., -1., 0.)).xy(),
            top_right: ndc_to_world.project_point3(vec3(1., 1., 0.)).xy(),
        };

        if let Some(mut buffer) = buffer {
            buffer.set(value);
            buffer.write_buffer(&device, &queue);
        } else {
            let mut buffer = BlitPixelizedShapesBuffer(UniformBuffer::from(value));
            buffer.write_buffer(&device, &queue);
            commands.entity(e).insert(buffer);
        }
    }
}

pub fn prepare_blit_pixelized_shape_pipelines(
    mut commands: Commands,
    shapes: Res<BlitPixelizedShapes>,
    cache: Res<PipelineCache>,
    mut cameras: Query<(
        Entity,
        &ViewTarget,
        &Msaa,
        Option<&mut BlitPixelizedShapesPipeline>,
    )>,
) {
    for (e, target, &msaa, mut pipeline) in &mut cameras {
        let create_pipeline = || BlitPixelizedShapesPipeline {
            format: target.main_texture_format(),
            msaa,
            id: cache.queue_render_pipeline(RenderPipelineDescriptor {
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
        };

        if let Some(pipeline) = pipeline.as_mut()
            && pipeline.format != target.main_texture_format()
            && pipeline.msaa != msaa
        {
            **pipeline = create_pipeline();
        } else if let None = pipeline {
            commands.entity(e).insert(create_pipeline());
        }
    }
}

impl<P: PhaseItem> FboWrappedDrawer<P> for BlitPixelizedShapes {
    type Param = (SRes<Self>, SRes<RenderDevice>, SRes<PipelineCache>);
    type ViewQuery = (
        Read<BlitPixelizedShapesBuffer>,
        Read<BlitPixelizedShapesPipeline>,
    );
    type ItemQuery = ();

    fn render<'w>(
        fbo: Fbo,
        _: &P,
        (buffer, pipeline): ROQueryItem<'w, Self::ViewQuery>,
        _: Option<ROQueryItem<'w, Self::ItemQuery>>,
        (shapes, device, pipeline_cache): SystemParamItem<'w, '_, Self::Param>,
        pass: &mut TrackedRenderPass<'w>,
    ) -> RenderCommandResult {
        let Some(pipeline) = pipeline_cache.into_inner().get_render_pipeline(pipeline.id) else {
            return RenderCommandResult::Skip;
        };
        let Some(buffer) = buffer.buffer() else {
            return RenderCommandResult::Skip;
        };

        pass.set_render_pipeline(pipeline);
        pass.wgpu_pass().set_bind_group(
            0,
            &device.create_bind_group(
                Some("centripetal_pixelized_shapes"),
                &shapes.fbo_layout,
                &BindGroupEntries::sequential((
                    &fbo.texture.default_view,
                    buffer.as_entire_buffer_binding(),
                )),
            ),
            &[],
        );
        pass.draw(0..3, 0..1);

        RenderCommandResult::Success
    }
}
