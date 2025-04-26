struct DrawInfo {
    viewport_size: vec2<u32>
}

struct Rect {
    min: vec2f,
    max: vec2f,
    color_tint: vec4f,
    color_uvwh: vec4f,
    alpha_uvwh: vec4f,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color_tint: vec4f,
    @location(1) color_uv: vec2f,
    @location(2) alpha_uv: vec2f,
};

@group(0) @binding(0) var<uniform> draw_info: DrawInfo;
@group(1) @binding(0) var<storage, read> rects: array<Rect>;

@vertex
fn vs_main(
    @builtin(vertex_index) in_vertex_index: u32,
) -> VertexOutput {
    let rect_index = in_vertex_index / 6;
    let rect = rects[rect_index];

    let vertex_index = in_vertex_index % 6;
    let vertex_position = rect.min + rect.max * CORNER_LOOKUP[vertex_index];

    var out: VertexOutput;

    out.clip_position = to_clip_coords(vertex_position);
    out.color_tint = rects[rect_index].color_tint;

    let color_uvwh = rects[rect_index].color_uvwh;
    out.color_uv = color_uvwh.xy + color_uvwh.zw * UV_LOOKUP[vertex_index];
    out.color_uv = vec2f(out.color_uv.x, out.color_uv.y);

    let alpha_uvwh = rects[rect_index].alpha_uvwh;
    out.alpha_uv = alpha_uvwh.xy + alpha_uvwh.zw * UV_LOOKUP[vertex_index];
    out.alpha_uv = vec2f(out.alpha_uv.x, out.alpha_uv.y);

    return out;
}

@group(2) @binding(0) var basic_sampler: sampler;
@group(3) @binding(0) var color_texture: texture_2d<f32>;
@group(3) @binding(1) var alpha_texture: texture_2d<f32>;

@fragment
fn fs_main(
    in: VertexOutput
) -> @location(0) vec4f {
    return in.color_tint
        * textureSample(color_texture, basic_sampler, in.color_uv)
        * vec4f(1.0, 1.0, 1.0, textureSample(alpha_texture, basic_sampler, in.alpha_uv).r);
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

// We flip the Y coordinate but also need to remap the UV coordinates from
// bottom-left to top-left. This lookup table keeps the UV coordinates the same,
// but flips the image upside down.
const UV_LOOKUP: array<vec2f, 6> = array<vec2f, 6>(
    vec2f(0.0, 1.0),
    vec2f(1.0, 0.0),
    vec2f(0.0, 0.0),
    vec2f(0.0, 1.0),
    vec2f(1.0, 1.0),
    vec2f(1.0, 0.0),
);

fn to_clip_coords(position: vec2f) -> vec4f {
    let xy = position / vec2f(f32(draw_info.viewport_size.x), f32(draw_info.viewport_size.y)) * 2.0 - 1.0;
    return vec4f(xy, 0.0, 1.0);
}
