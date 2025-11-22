use crate::{
    prelude::*,
    render::painter::{Blending, Painter, PainterVertexBuffer, RenderPainter, VERTEX_ATTRIBUTES, Vertex},
};

#[derive(Resource, Debug)]
pub struct PainterPipeline {
    view_layout: BindGroupLayout,
    material_layout: BindGroupLayout,
    vertex_shader: Handle<Shader>,
    default_fragment_shader: Handle<Shader>,
}

fn init_pipeline(mut commands: Commands, server: Res<AssetServer>, device: Res<RenderDevice>) {
    let [tonemapping_texture_entry, tonemapping_sampler_entry] = get_lut_bind_group_layout_entries();
    let view_layout = device.create_bind_group_layout(
        Some("painter_view_layout"),
        &BindGroupLayoutEntries::sequential(
            ShaderStages::VERTEX_FRAGMENT,
            (
                uniform_buffer::<ViewUniform>(true),
                tonemapping_texture_entry.visibility(ShaderStages::FRAGMENT),
                tonemapping_sampler_entry.visibility(ShaderStages::FRAGMENT),
            ),
        ),
    );

    let material_layout = device.create_bind_group_layout(
        Some("painter_material_layout"),
        &BindGroupLayoutEntries::sequential(
            ShaderStages::FRAGMENT,
            (
                texture_2d(TextureSampleType::Float { filterable: true }),
                sampler(SamplerBindingType::Filtering),
            ),
        ),
    );

    let server = server.into_inner();
    commands.insert_resource(PainterPipeline {
        view_layout,
        material_layout,
        vertex_shader: server.load("shaders/painter/pipeline.wgsl"),
        default_fragment_shader: server.load("shaders/painter/default.wgsl"),
    });
}

bitflags! {
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    #[repr(transparent)]
    pub struct PainterPipelineKey: u32 {
        const NONE           = 0;
        const BLEND_ADDITIVE = 1 << 0;
    }
}

impl PainterPipelineKey {
    pub const BLEND_BITS: Self = Self::BLEND_ADDITIVE;

    pub const fn from_blend(blend: Blending) -> Self {
        match blend {
            Blending::Normal => Self::NONE,
            Blending::Additive => Self::BLEND_ADDITIVE,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq)]
#[repr(C, align(8))]
pub struct PainterPipelineKeys {
    pub sprite_key: SpritePipelineKey,
    pub painter_key: PainterPipelineKey,
}

impl PainterPipelineKeys {
    #[inline(always)]
    pub const fn to_bits(self) -> u64 {
        self.sprite_key.bits() as u64 | ((self.painter_key.bits() as u64) << 32)
    }
}

impl PartialEq for PainterPipelineKeys {
    fn eq(&self, other: &Self) -> bool {
        self.to_bits() == other.to_bits()
    }
}

impl Hash for PainterPipelineKeys {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.to_bits().hash(state);
    }
}

impl SpecializedRenderPipeline for PainterPipeline {
    type Key = PainterPipelineKeys;

    fn specialize(&self, PainterPipelineKeys { sprite_key, painter_key }: Self::Key) -> RenderPipelineDescriptor {
        let mut shader_defs: Vec<ShaderDefVal> = Vec::new();
        if sprite_key.contains(SpritePipelineKey::TONEMAP_IN_SHADER) {
            shader_defs.push("TONEMAP_IN_SHADER".into());
            shader_defs.push(ShaderDefVal::UInt("TONEMAPPING_LUT_TEXTURE_BINDING_INDEX".into(), 1));
            shader_defs.push(ShaderDefVal::UInt("TONEMAPPING_LUT_SAMPLER_BINDING_INDEX".into(), 2));

            let method = sprite_key.intersection(SpritePipelineKey::TONEMAP_METHOD_RESERVED_BITS);
            shader_defs.push(
                bitflags_match!(method, {
                    SpritePipelineKey::TONEMAP_METHOD_REINHARD => "TONEMAP_METHOD_REINHARD",
                    SpritePipelineKey::TONEMAP_METHOD_REINHARD_LUMINANCE => "TONEMAP_METHOD_REINHARD_LUMINANCE",
                    SpritePipelineKey::TONEMAP_METHOD_ACES_FITTED => "TONEMAP_METHOD_ACES_FITTED",
                    SpritePipelineKey::TONEMAP_METHOD_AGX => "TONEMAP_METHOD_AGX",
                    SpritePipelineKey::TONEMAP_METHOD_SOMEWHAT_BORING_DISPLAY_TRANSFORM => "TONEMAP_METHOD_SOMEWHAT_BORING_DISPLAY_TRANSFORM",
                    SpritePipelineKey::TONEMAP_METHOD_BLENDER_FILMIC => "TONEMAP_METHOD_BLENDER_FILMIC",
                    SpritePipelineKey::TONEMAP_METHOD_TONY_MC_MAPFACE => "TONEMAP_METHOD_TONY_MC_MAPFACE",
                    _ => "TONEMAP_METHOD_NONE"
                })
                .into(),
            );

            if sprite_key.contains(SpritePipelineKey::DEBAND_DITHER) {
                shader_defs.push("DEBAND_DITHER".into());
            }
        }

        let blend = painter_key.intersection(PainterPipelineKey::BLEND_BITS);
        let format = match sprite_key.contains(SpritePipelineKey::HDR) {
            true => ViewTarget::TEXTURE_FORMAT_HDR,
            false => TextureFormat::bevy_default(),
        };

        RenderPipelineDescriptor {
            label: Some("painter_pipeline".into()),
            layout: vec![self.view_layout.clone(), self.material_layout.clone()],
            push_constant_ranges: vec![],
            vertex: VertexState {
                shader: self.vertex_shader.clone(),
                shader_defs: shader_defs.clone(),
                entry_point: Some("vertex".into()),
                buffers: vec![VertexBufferLayout {
                    array_stride: size_of::<Vertex>() as BufferAddress,
                    step_mode: VertexStepMode::Vertex,
                    attributes: VERTEX_ATTRIBUTES.into(),
                }],
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
                count: sprite_key.msaa_samples(),
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            fragment: Some(FragmentState {
                shader: self.default_fragment_shader.clone(),
                shader_defs,
                entry_point: Some("fragment".into()),
                targets: vec![Some(ColorTargetState {
                    format,
                    blend: Some(bitflags_match!(blend, {
                        PainterPipelineKey::BLEND_ADDITIVE => BlendState {
                            color: BlendComponent {
                                src_factor: BlendFactor::SrcAlpha,
                                dst_factor: BlendFactor::One,
                                operation: BlendOperation::Add,
                            },
                            alpha: BlendComponent::OVER,
                        },
                        _ => BlendState::ALPHA_BLENDING,
                    })),
                    write_mask: ColorWrites::ALL,
                })],
            }),
            zero_initialize_workgroup_memory: false,
        }
    }
}

#[derive(Resource, Default)]
struct ImageMessages(Vec<AssetEvent<Image>>);

fn extract_image_messages(mut extracted_messages: Extract<MessageReader<AssetEvent<Image>>>, mut messages: ResMut<ImageMessages>) {
    messages.0.extend(extracted_messages.read());
}

fn queue_painters(
    draw_functions: Res<DrawFunctions<Transparent2d>>,
    painter_pipeline: Res<PainterPipeline>,
    pipelines: ResMut<SpecializedRenderPipelines<PainterPipeline>>,
    pipeline_cache: Res<PipelineCache>,
    mut transparent_render_phases: ResMut<ViewSortedRenderPhases<Transparent2d>>,
    views: Query<(&RenderVisibleEntities, &ExtractedView, &Msaa, Option<&Tonemapping>, Option<&DebandDither>)>,
    painters: Query<&RenderPainter>,
) {
    let pipelines = pipelines.into_inner();
    let draw_function = draw_functions.read().id::<DrawPainter>();

    for (visible_entities, view, msaa, tonemapping, dither) in &views {
        let Some(transparent_phase) = transparent_render_phases.get_mut(&view.retained_view_entity) else { continue };

        let mut sprite_key = SpritePipelineKey::from_hdr(view.hdr) | SpritePipelineKey::from_msaa_samples(msaa.samples());
        if !view.hdr {
            if let Some(tonemapping) = tonemapping {
                sprite_key |= SpritePipelineKey::TONEMAP_IN_SHADER;
                sprite_key |= match tonemapping {
                    Tonemapping::None => SpritePipelineKey::TONEMAP_METHOD_NONE,
                    Tonemapping::Reinhard => SpritePipelineKey::TONEMAP_METHOD_REINHARD,
                    Tonemapping::ReinhardLuminance => SpritePipelineKey::TONEMAP_METHOD_REINHARD_LUMINANCE,
                    Tonemapping::AcesFitted => SpritePipelineKey::TONEMAP_METHOD_ACES_FITTED,
                    Tonemapping::AgX => SpritePipelineKey::TONEMAP_METHOD_AGX,
                    Tonemapping::SomewhatBoringDisplayTransform => SpritePipelineKey::TONEMAP_METHOD_SOMEWHAT_BORING_DISPLAY_TRANSFORM,
                    Tonemapping::TonyMcMapface => SpritePipelineKey::TONEMAP_METHOD_TONY_MC_MAPFACE,
                    Tonemapping::BlenderFilmic => SpritePipelineKey::TONEMAP_METHOD_BLENDER_FILMIC,
                };
            }
            if let Some(DebandDither::Enabled) = dither {
                sprite_key |= SpritePipelineKey::DEBAND_DITHER;
            }
        }

        for &(painter_entity, painter_main_entity) in visible_entities.iter::<Painter>() {
            let Ok(painter) = painters.get(painter_entity) else { continue };
            for (extracted_index, &(.., key)) in painter.requests.iter().enumerate() {
                let pipeline = pipelines.specialize(&pipeline_cache, &painter_pipeline, PainterPipelineKeys {
                    sprite_key,
                    painter_key: PainterPipelineKey::from_blend(key.blend),
                });

                transparent_phase.add(Transparent2d {
                    draw_function,
                    pipeline,
                    entity: (painter_entity, painter_main_entity),
                    sort_key: key.layer,
                    batch_range: 0..0,
                    extra_index: PhaseItemExtraIndex::None,
                    extracted_index,
                    indexed: true,
                });
            }
        }
    }
}

#[derive(Component)]
pub struct PainterViewBindGroup {
    pub value: BindGroup,
}

fn prepare_painter_view_bind_groups(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    painter_pipeline: Res<PainterPipeline>,
    view_uniforms: Res<ViewUniforms>,
    views: Query<(Entity, &Tonemapping), With<ExtractedView>>,
    tonemapping_luts: Res<TonemappingLuts>,
    images: Res<RenderAssets<GpuImage>>,
    fallback_image: Res<FallbackImage>,
) {
    let Some(view_binding) = view_uniforms.uniforms.binding() else {
        return;
    };

    for (entity, tonemapping) in &views {
        let lut_bindings = get_lut_bindings(&images, &tonemapping_luts, tonemapping, &fallback_image);
        let view_bind_group = render_device.create_bind_group(
            "painter_view_bind_group",
            &painter_pipeline.view_layout,
            &BindGroupEntries::sequential((view_binding.clone(), lut_bindings.0, lut_bindings.1)),
        );

        commands.entity(entity).insert(PainterViewBindGroup { value: view_bind_group });
    }
}

#[derive(Resource)]
pub struct FallbackPainterBindGroup(BindGroup);

fn prepare_fallback_bind_group(mut commands: Commands, fallback: Res<FallbackImage>, device: Res<RenderDevice>, pipeline: Res<PainterPipeline>) {
    commands.insert_resource(FallbackPainterBindGroup(device.create_bind_group(
        Some("painter_material_fallback_bind_group"),
        &pipeline.material_layout,
        &BindGroupEntries::sequential((&fallback.d2.texture_view, &fallback.d2.sampler)),
    )));
}

#[derive(Resource, Default)]
pub struct PainterBindGroups(HashMap<AssetId<Image>, BindGroup>);

#[derive(Resource)]
pub struct PainterIndices(RawBufferVec<u32>);
impl Default for PainterIndices {
    fn default() -> Self {
        Self(RawBufferVec::new(BufferUsages::INDEX | BufferUsages::COPY_DST))
    }
}

#[derive(Resource, Default)]
pub struct PainterBatches(HashMap<(RetainedViewEntity, Entity), PainterBatch>);

#[derive(PartialEq, Eq, Clone, Debug)]
struct PainterBatch {
    image_handle_id: AssetId<Image>,
    range: Range<u32>,
}

fn prepare_painters(
    device: Res<RenderDevice>,
    queue: Res<RenderQueue>,
    mut messages: ResMut<ImageMessages>,
    mut image_bind_groups: ResMut<PainterBindGroups>,
    gpu_images: Res<RenderAssets<GpuImage>>,
    painter_pipeline: Res<PainterPipeline>,
    mut indices: ResMut<PainterIndices>,
    mut batches: ResMut<PainterBatches>,
    mut phases: ResMut<ViewSortedRenderPhases<Transparent2d>>,
    painters: Query<&RenderPainter>,
) {
    for msg in messages.0.drain(..) {
        match msg {
            AssetEvent::Added { .. } | AssetEvent::LoadedWithDependencies { .. } => {}
            AssetEvent::Unused { id } | AssetEvent::Modified { id } | AssetEvent::Removed { id } => {
                image_bind_groups.0.remove(&id);
            }
        };
    }

    batches.0.clear();
    indices.0.clear();

    let mut index = 0;
    for (&retained_view, transparent_phase) in phases.iter_mut() {
        let mut current_batch = None;
        let mut batch_item_index = 0;
        let mut batch_image_handle = AssetId::invalid();

        for item_index in 0..transparent_phase.items.len() {
            let item = &transparent_phase.items[item_index];
            let Ok(painter) = painters.get(item.entity()) else {
                batch_image_handle = AssetId::invalid();
                continue
            };

            let Some(&(quad_index, key)) = painter.requests.get(item.extracted_index) else {
                batch_image_handle = AssetId::invalid();
                continue
            };

            if batch_image_handle != key.image {
                batch_image_handle = key.image;
                let Some(gpu_image) = gpu_images.get(batch_image_handle) else { continue };

                image_bind_groups.0.entry(batch_image_handle).or_insert_with(|| {
                    device.create_bind_group(
                        "painter_material_bind_group",
                        &painter_pipeline.material_layout,
                        &BindGroupEntries::sequential((&gpu_image.texture_view, &gpu_image.sampler)),
                    )
                });

                batch_item_index = item_index;
                current_batch = Some(batches.0.entry((retained_view, item.entity())).insert(PainterBatch {
                    image_handle_id: batch_image_handle,
                    range: index..index,
                }));
            }

            indices
                .0
                .extend([quad_index, quad_index + 1, quad_index + 2, quad_index + 2, quad_index + 3, quad_index].map(|i| i as u32));

            transparent_phase.items[batch_item_index].batch_range.end += 1;
            current_batch.as_mut().unwrap().get_mut().range.end += 6;
            index += 6;
        }
    }

    indices.0.write_buffer(&device, &queue);
}

pub type DrawPainter = (
    SetItemPipeline,
    SetPainterViewBindGroup<0>,
    SetPainterTextureBindGroup<1>,
    DrawPainterBatch,
);

pub struct SetPainterViewBindGroup<const I: usize>;
impl<P: PhaseItem, const I: usize> RenderCommand<P> for SetPainterViewBindGroup<I> {
    type Param = ();
    type ViewQuery = (Read<ViewUniformOffset>, Read<PainterViewBindGroup>);
    type ItemQuery = ();

    fn render<'w>(
        _item: &P,
        (view_uniform, painter_view_bind_group): ROQueryItem<'w, '_, Self::ViewQuery>,
        _entity: Option<()>,
        _param: SystemParamItem<'w, '_, Self::Param>,
        pass: &mut TrackedRenderPass<'w>,
    ) -> RenderCommandResult {
        pass.set_bind_group(I, &painter_view_bind_group.value, &[view_uniform.offset]);
        RenderCommandResult::Success
    }
}

pub struct SetPainterTextureBindGroup<const I: usize>;
impl<P: PhaseItem, const I: usize> RenderCommand<P> for SetPainterTextureBindGroup<I> {
    type Param = (SRes<PainterBindGroups>, SRes<FallbackPainterBindGroup>, SRes<PainterBatches>);
    type ViewQuery = Read<ExtractedView>;
    type ItemQuery = ();

    fn render<'w>(
        item: &P,
        view: ROQueryItem<'w, '_, Self::ViewQuery>,
        _entity: Option<()>,
        (image_bind_groups, fallback_image_bind_group, batches): SystemParamItem<'w, '_, Self::Param>,
        pass: &mut TrackedRenderPass<'w>,
    ) -> RenderCommandResult {
        let image_bind_groups = image_bind_groups.into_inner();
        let Some(batch) = batches.0.get(&(view.retained_view_entity, item.entity())) else { return RenderCommandResult::Skip };
        let bind_group = if batch.image_handle_id == AssetId::default() {
            &fallback_image_bind_group.into_inner().0
        } else if let Some(bind_group) = image_bind_groups.0.get(&batch.image_handle_id) {
            bind_group
        } else {
            return RenderCommandResult::Skip
        };

        pass.set_bind_group(I, bind_group, &[]);
        RenderCommandResult::Success
    }
}

pub struct DrawPainterBatch;
impl<P: PhaseItem> RenderCommand<P> for DrawPainterBatch {
    type Param = (SRes<PainterVertexBuffer>, SRes<PainterIndices>, SRes<PainterBatches>);
    type ViewQuery = Read<ExtractedView>;
    type ItemQuery = ();

    fn render<'w>(
        item: &P,
        view: ROQueryItem<'w, '_, Self::ViewQuery>,
        _entity: Option<()>,
        (vertex_buffer, indices, batches): SystemParamItem<'w, '_, Self::Param>,
        pass: &mut TrackedRenderPass<'w>,
    ) -> RenderCommandResult {
        let vertex_buffer = &vertex_buffer.into_inner().vertex_buffer;
        let indices = &indices.into_inner().0;
        let Some(batch) = batches.0.get(&(view.retained_view_entity, item.entity())) else { return RenderCommandResult::Skip };

        pass.set_index_buffer(indices.buffer().unwrap().slice(..), 0, IndexFormat::Uint32);
        pass.set_vertex_buffer(0, vertex_buffer.slice(..));
        pass.draw_indexed(batch.range.clone(), 0, 0..1);
        RenderCommandResult::Success
    }
}

pub(super) fn plugin(app: &mut App) {
    if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
        render_app
            .init_resource::<SpecializedRenderPipelines<PainterPipeline>>()
            .init_resource::<ImageMessages>()
            .init_resource::<PainterBindGroups>()
            .init_resource::<PainterIndices>()
            .init_resource::<PainterBatches>()
            .add_render_command::<Transparent2d, DrawPainter>()
            .add_systems(RenderStartup, init_pipeline)
            .add_systems(ExtractSchedule, extract_image_messages)
            .add_systems(
                Render,
                (
                    queue_painters.in_set(RenderSystems::Queue),
                    (
                        prepare_fallback_bind_group.run_if(resource_changed::<FallbackImage>),
                        (prepare_painters, prepare_painter_view_bind_groups),
                    )
                        .chain()
                        .in_set(RenderSystems::PrepareBindGroups),
                ),
            );
    }
}
