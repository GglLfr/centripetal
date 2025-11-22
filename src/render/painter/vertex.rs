use crate::prelude::*;

#[derive(Pod, Zeroable, Debug, Clone, Copy)]
#[repr(C)]
pub struct Vertex {
    pub position: Vec2,
    pub color: LinearRgba,
    pub uv: Vec2,
}

impl Vertex {
    pub fn new(position: impl Into<Vec2>, color: impl Into<LinearRgba>, uv: impl Into<Vec2>) -> Self {
        Self {
            position: position.into(),
            color: color.into(),
            uv: uv.into(),
        }
    }
}

pub const VERTEX_ATTRIBUTES: &'static [VertexAttribute] = &[
    VertexAttribute {
        format: VertexFormat::Float32x2,
        offset: offset_of!(Vertex, position) as BufferAddress,
        shader_location: 0,
    },
    VertexAttribute {
        format: VertexFormat::Float32x4,
        offset: offset_of!(Vertex, color) as BufferAddress,
        shader_location: 1,
    },
    VertexAttribute {
        format: VertexFormat::Float32x2,
        offset: offset_of!(Vertex, uv) as BufferAddress,
        shader_location: 2,
    },
];

#[derive(Debug, Clone, Copy)]
pub struct RequestKey {
    pub image: AssetId<Image>,
    pub blend: Blending,
    pub layer: FloatOrd,
}

impl Default for RequestKey {
    fn default() -> Self {
        Self {
            image: default(),
            blend: default(),
            layer: FloatOrd(0.),
        }
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub enum Blending {
    #[default]
    Normal,
    Additive,
}

#[derive(Component, Debug)]
#[require(Transform, Visibility, VisibilityClass)]
#[component(on_add = add_visibility_class::<Painter>)]
pub struct Painter {
    requests: VecBelt<(usize, RequestKey)>,
}

impl Default for Painter {
    fn default() -> Self {
        Self { requests: VecBelt::new(1) }
    }
}

#[derive(Resource, Debug)]
pub struct PainterQuads {
    quads: VecBelt<[Vertex; 4]>,
}

impl Default for PainterQuads {
    fn default() -> Self {
        Self { quads: VecBelt::new(8192) }
    }
}

impl PainterQuads {
    pub fn request(&self, painter: &Painter, image: impl Into<AssetId<Image>>, blend: Blending, layer: f32, quads: impl Transfer<[Vertex; 4]>) {
        let len = quads.len();
        let key = RequestKey {
            image: image.into(),
            blend,
            layer: FloatOrd(layer),
        };

        let first = self.quads.append(quads);
        painter.requests.append(unsafe {
            vec_belt::transfer(len, |ptr: *mut (usize, RequestKey)| {
                for i in 0..len {
                    ptr.add(i).write((first.unchecked_add(i), key));
                }
            })
        });
    }
}

#[derive(Resource, Debug, Deref, DerefMut)]
pub struct PainterVertexBuffer {
    pub vertex_buffer: Buffer,
}

fn init_quads(mut commands: Commands, device: Res<RenderDevice>) {
    commands.insert_resource(PainterVertexBuffer {
        vertex_buffer: device.create_buffer(&BufferDescriptor {
            label: Some("painter_vertex_buffer"),
            size: size_of::<[[Vertex; 4]; 8192]>() as BufferAddress,
            usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        }),
    });
}

#[derive(Component, Debug, Default, Clone)]
pub struct RenderPainter {
    pub requests: SmallVec<[(usize, RequestKey); 8]>,
}

fn extract_quads_and_painters(
    mut commands: Commands,
    mut main_world: ResMut<MainWorld>,
    device: Res<RenderDevice>,
    queue: Res<RenderQueue>,
    mut painter_buffer: ResMut<PainterVertexBuffer>,
    mut render_painters: Query<&mut RenderPainter>,
    mut state: Local<Option<SystemState<(ResMut<PainterQuads>, Query<(RenderEntity, &mut Painter)>)>>>,
    mut new_render_painters: Local<Vec<(Entity, RenderPainter)>>,
) {
    let (mut painter_quads, painters) = state.get_or_insert_with(|| SystemState::new(&mut main_world)).get_mut(&mut main_world);
    ComputeTaskPool::get().scope(|scope| {
        scope.spawn(async move {
            painter_quads.quads.clear(|slice| {
                let slice: &[u8] = cast_slice(&slice);
                if painter_buffer.size() < slice.len() as BufferAddress {
                    **painter_buffer = device.create_buffer(&BufferDescriptor {
                        label: Some("painter_vertex_buffer"),
                        size: (painter_buffer.size() * 2)
                            .max(slice.len() as BufferAddress)
                            .next_multiple_of(COPY_BUFFER_ALIGNMENT),
                        usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
                        mapped_at_creation: true,
                    });

                    painter_buffer
                        .get_mapped_range_mut(0..slice.len() as BufferAddress)
                        .copy_from_slice(slice);
                    painter_buffer.vertex_buffer.unmap();
                } else {
                    queue.write_buffer(&painter_buffer, 0, slice);
                }
            });
        });

        scope.spawn(async move {
            for (render_entity, mut painter) in painters {
                painter.requests.clear(|slice| match render_painters.get_mut(render_entity) {
                    Ok(mut render_painter) => {
                        render_painter.requests.clear();
                        render_painter.requests.extend_from_slice(&slice);
                    }
                    Err(..) => new_render_painters.push((render_entity, RenderPainter {
                        requests: SmallVec::from_slice(&slice),
                    })),
                });
            }
            commands.insert_batch(mem::take(&mut *new_render_painters));
        });
    });
}

pub(super) fn plugin(app: &mut App) {
    app.add_plugins(SyncComponentPlugin::<Painter>::default()).init_resource::<PainterQuads>();

    if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
        render_app
            .add_systems(RenderStartup, init_quads)
            .add_systems(ExtractSchedule, extract_quads_and_painters);
    }
}
