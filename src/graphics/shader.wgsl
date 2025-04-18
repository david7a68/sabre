struct DrawInfo {
    viewport_size: vec2<u32>
}

struct Rect {
    min: vec2f,
    max: vec2f,
    uvwh: vec4f,
    color: vec4f,
    texture: u32,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4f,
};

@group(0) @binding(0)
var<uniform> draw_info: DrawInfo;

@group(1) @binding(0)
var<storage, read> rects: array<Rect>;

@vertex
fn vs_main(
    @builtin(vertex_index) in_vertex_index: u32,
) -> VertexOutput {
    var out: VertexOutput;

    let rect_index = in_vertex_index / 6;
    let rect = rects[rect_index];

    let vertex_index = in_vertex_index % 6;
    let vertex_position = rect.min + rect.max * CORNER_LOOKUP[vertex_index];

    out.clip_position = to_clip_coords(vertex_position);
    out.color = rects[rect_index].color;

    return out;
}

@fragment
fn fs_main(
    in: VertexOutput
) -> @location(0) vec4f {
    return in.color;    
}

/// 2----1  5
/// |   / / |
/// |  / /  |
/// | / /   |
/// 0  3----4
const CORNER_LOOKUP: array<vec2f, 6> = array<vec2f, 6>(
    vec2f(0.0, 0.0),
    vec2f(1.0, 1.0),
    vec2f(0.0, 1.0),
    vec2f(0.0, 0.0),
    vec2f(1.0, 0.0),
    vec2f(1.0, 1.0),
);

fn to_clip_coords(position: vec2f) -> vec4f {
    let xy = position / vec2f(f32(draw_info.viewport_size.x), f32(draw_info.viewport_size.y)) * 2.0 - 1.0;
    return vec4f(xy, 0.0, 1.0);
}
