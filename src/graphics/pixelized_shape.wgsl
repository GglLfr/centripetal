const POSITIONS = array(
    vec4(-1., -3., 0., 1.),
    vec4(3., 1., 0., 1.),
    vec4(-1., 1., 0., 1.),
);

const UVS = array(
    vec2(0., 2.),
    vec2(2., 0.,),
    vec2(0., 0.),
);

struct VertexOut {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vertex_main(@builtin(vertex_index) vertex_index: u32) -> VertexOut {
    var out: VertexOut;
    out.position = POSITIONS[vertex_index];
    out.uv = UVS[vertex_index];
    return out;
}

@group(0) @binding(0) var sprite_texture: texture_2d<f32>;
@group(0) @binding(1) var sprite_sampler: sampler;

@fragment
fn fragment_main(in: VertexOut) -> @location(0) vec4<f32> {
    return textureSample(sprite_texture, sprite_sampler, in.uv);
}
