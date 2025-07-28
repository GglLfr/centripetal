struct BlitPixelizedShapesBufferValue {
    bottom_left: vec2<f32>,
    top_right: vec2<f32>,
}

struct VertexOut {
    @builtin(position) position: vec4<f32>,
    @location(0) world_pos: vec2<f32>,
}

@group(0) @binding(0) var fbo_texture: texture_2d<f32>;
@group(0) @binding(1) var<uniform> buffer: BlitPixelizedShapesBufferValue;

@vertex
fn vertex_main(@builtin(vertex_index) vertex_index: u32) -> VertexOut {
    let bl = buffer.bottom_left;
    let tr = buffer.top_right;

    var out: VertexOut;
    switch i32(vertex_index) {
        case 0: {
            out.position = vec4(-1., -3., 0., 1.);
            out.world_pos = vec2(bl.x, bl.y - (tr.y - bl.y));
        }
        case 1: {
            out.position = vec4(3., 1., 0., 1.);
            out.world_pos = vec2(tr.x + (tr.x - bl.x), tr.y);
        }
        case 2: {
            out.position = vec4(-1., 1., 0., 1.);
            out.world_pos = vec2(bl.x, tr.y);
        }
        default: {}
    }
    
    return out;
}

@fragment
fn fragment_main(in: VertexOut) -> @location(0) vec4<f32> {
    let bl = buffer.bottom_left;
    let tr = buffer.top_right;
    let dims = tr - bl;
    let dims_pixel = vec2<f32>(textureDimensions(fbo_texture));

    let from_world = floor(in.world_pos);
    let to_world = ceil(in.world_pos);

    let from_pixel = (from_world - bl) / dims * dims_pixel;
    let to_pixel = (to_world - bl) / dims * dims_pixel;

    var count = 0;
    var color = vec4(0., 0., 0., 0.);
    for(var y = floor(from_pixel.y); y <= ceil(from_pixel.y); y += 1) {
        for(var x = floor(from_pixel.x); x <= ceil(from_pixel.x); x += 1) {
            count += 1;
            color += textureLoad(fbo_texture, vec2(u32(x), u32(dims_pixel.y - y)), 0);
        }
    }

    return color / f32(count);
}
