struct DrawInfo {
    viewport_size: vec2<u32>
}

struct Rect {
    point: vec2f,
    extent: vec2f,
    color_tint: vec4f,
    color_uvwh: vec4f,
    alpha_uvwh: vec4f,
    use_nearest_sampling: u32,
    _padding0: u32,
    _padding1: u32,
    _padding2: u32,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color_tint: vec4f,
    @location(1) color_uv: vec2f,
    @location(2) alpha_uv: vec2f,
    @location(3) use_nearest_sampling: u32,
};

@group(0) @binding(0) var<uniform> draw_info: DrawInfo;
@group(0) @binding(1) var<storage, read> rects: array<Rect>;

@vertex
fn vs_main(
    @builtin(vertex_index) in_vertex_index: u32,
) -> VertexOutput {
    let rect_index = in_vertex_index / 6;
    let rect = rects[rect_index];

    let vertex_index = in_vertex_index % 6;
    let vertex_position = rect.point + rect.extent * CORNER_LOOKUP[vertex_index];

    var out: VertexOutput;

    out.clip_position = to_clip_coords(vertex_position);
    out.color_tint = rects[rect_index].color_tint;

    let color_uvwh = rects[rect_index].color_uvwh;
    out.color_uv = color_uvwh.xy + color_uvwh.zw * UV_LOOKUP[vertex_index];
    out.color_uv = vec2f(out.color_uv.x, out.color_uv.y);

    let alpha_uvwh = rects[rect_index].alpha_uvwh;
    out.alpha_uv = alpha_uvwh.xy + alpha_uvwh.zw * UV_LOOKUP[vertex_index];
    out.alpha_uv = vec2f(out.alpha_uv.x, out.alpha_uv.y);

    out.use_nearest_sampling = rects[rect_index].use_nearest_sampling;

    return out;
}

@group(1) @binding(0) var basic_sampler: sampler;
@group(1) @binding(1) var nearest_sampler: sampler;
@group(2) @binding(0) var color_texture: texture_2d<f32>;
@group(2) @binding(1) var alpha_texture: texture_2d<f32>;

@fragment
fn fs_main(
    in: VertexOutput
) -> @location(0) vec4f {
    var color: vec4f;
    var alpha: f32;
    
    if (in.use_nearest_sampling != 0u) {
        color = in.color_tint * textureSample(color_texture, nearest_sampler, in.color_uv);
        alpha = textureSample(alpha_texture, nearest_sampler, in.alpha_uv).r;
    } else {
        color = in.color_tint * textureSample(color_texture, basic_sampler, in.color_uv);
        alpha = textureSample(alpha_texture, basic_sampler, in.alpha_uv).r;
    }
    
    color.a = alpha;
    return color;
}

/// Triangle layout for top-left origin (Y down):
/// 1----2  4
/// |   / / |
/// |  / /  |
/// | / /   |
/// 0  3----5
const CORNER_LOOKUP: array<vec2f, 6> = array<vec2f, 6>(
    vec2f(0.0, 0.0),
    vec2f(0.0, 1.0),
    vec2f(1.0, 1.0),
    vec2f(0.0, 0.0),
    vec2f(1.0, 1.0),
    vec2f(1.0, 0.0),
);

// UV coordinates for top-left origin with Y pointing down
const UV_LOOKUP: array<vec2f, 6> = array<vec2f, 6>(
    vec2f(0.0, 0.0),
    vec2f(0.0, 1.0),
    vec2f(1.0, 1.0),
    vec2f(0.0, 0.0),
    vec2f(1.0, 1.0),
    vec2f(1.0, 0.0),
);

fn to_clip_coords(position: vec2f) -> vec4f {
    let x = position.x / f32(draw_info.viewport_size.x) * 2.0 - 1.0;
    let y = -(position.y / f32(draw_info.viewport_size.y) * 2.0 - 1.0);
    return vec4f(x, y, 0.0, 1.0);
}
