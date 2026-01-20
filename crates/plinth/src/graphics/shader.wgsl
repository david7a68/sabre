struct DrawInfo {
    viewport_size: vec2<u32>
}

// Rectangle primitive with configurable paint (sampled texture or gradient)
struct Rect {
    point: vec2f,
    extent: vec2f,
    background: Paint,
    border_color: GradientPaint,
    border_width: vec4f,
    control_flags: Bitflags,
    _padding0: u32,
    _padding1: u32,
    _padding2: u32,
}

struct VertexOutput {
    @builtin(position) frag_coord: vec4<f32>,
    @location(0) @interpolate(flat) rect_index: u32,
    @location(1) @interpolate(flat) border_left: f32,
    @location(2) @interpolate(flat) border_top: f32,
    @location(3) @interpolate(flat) border_right: f32,
    @location(4) @interpolate(flat) border_bottom: f32,
    @location(5) uv: vec2f,
};

// Bind group 0: per-frame info
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

    let border_left = rect.point.x + rect.border_width.x;
    let border_top = rect.point.y + rect.border_width.y;
    let border_right = rect.point.x + rect.extent.x - rect.border_width.z;
    let border_bottom = rect.point.y + rect.extent.y - rect.border_width.w;

    var out: VertexOutput;

    out.rect_index = rect_index;
    out.frag_coord = to_clip_coords(vertex_position);
    out.uv = UV_LOOKUP[vertex_index];

    out.border_left = border_left;
    out.border_top = border_top;
    out.border_right = border_right;
    out.border_bottom = border_bottom;

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
    let rect = rects[in.rect_index];
    var color: vec4f;

    let is_in_border =
        in.frag_coord.x < in.border_left ||
        in.frag_coord.x > in.border_right ||
        in.frag_coord.y < in.border_top ||
        in.frag_coord.y > in.border_bottom;

    if (is_in_border) {
         // Gradient paint mode
        let gradient = rect.border_color;

        // Calculate gradient interpolation factor based on position
        let p1 = gradient.color_p1;
        let p2 = gradient.color_p2;
        let gradient_dir = p2 - p1;
        let gradient_len_sq = dot(gradient_dir, gradient_dir);
        
        // Current position in normalized [0,1] space within the rect
        let pos = in.uv;
        
        // Project position onto gradient line to get interpolation factor
        var t: f32;
        if (gradient_len_sq < 0.0001) {
            t = 0.0;
        } else {
            t = clamp(dot(pos - p1, gradient_dir) / gradient_len_sq, 0.0, 1.0);
        }
        
        color = mix(gradient.color_a, gradient.color_b, t);
    } else if (is_gradient_paint(rect.control_flags)) {
        // Gradient paint mode
        let gradient = as_gradient_paint(rect.background);
        
        // Calculate gradient interpolation factor based on position
        let p1 = gradient.color_p1;
        let p2 = gradient.color_p2;
        let gradient_dir = p2 - p1;
        let gradient_len_sq = dot(gradient_dir, gradient_dir);
        
        // Current position in normalized [0,1] space within the rect
        let pos = in.uv;
        
        // Project position onto gradient line to get interpolation factor
        var t: f32;
        if (gradient_len_sq < 0.0001) {
            t = 0.0;
        } else {
            t = clamp(dot(pos - p1, gradient_dir) / gradient_len_sq, 0.0, 1.0);
        }
        
        color = mix(gradient.color_a, gradient.color_b, t);
    } else {
        // Sampled texture mode
        let sampled = as_sampled_paint(rect.background);
        
        let color_uv = sampled.color_uvwh.xy + sampled.color_uvwh.zw * in.uv;
        let alpha_uv = sampled.alpha_uvwh.xy + sampled.alpha_uvwh.zw * in.uv;

        if (is_nearest_sampling(rect.control_flags)) {
            color = sampled.color_tint * textureSample(color_texture, nearest_sampler, color_uv);
            color.a *= textureSample(alpha_texture, nearest_sampler, alpha_uv).r;
        } else {
            color = sampled.color_tint * textureSample(color_texture, basic_sampler, color_uv);
            color.a *= textureSample(alpha_texture, basic_sampler, alpha_uv).r;
        }
    }
    
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

const USE_NEAREST_SAMPLING: u32 = 1;
const USE_GRADIENT_PAINT: u32 = 2;

struct Bitflags {
    value: u32
}

fn is_nearest_sampling(flags: Bitflags) -> bool {
    return (flags.value & USE_NEAREST_SAMPLING) != 0u;
}

fn is_gradient_paint(flags: Bitflags) -> bool {
    return (flags.value & USE_GRADIENT_PAINT) != 0u;
}

struct Paint {
    a: vec4f,
    b: vec4f,
    c: vec4f,
}

fn as_sampled_paint(paint: Paint) -> SampledPaint {
    var result = SampledPaint();
    result.color_tint = paint.a;
    result.color_uvwh = paint.b;
    result.alpha_uvwh = paint.c;

    return result;
}

fn as_gradient_paint(paint: Paint) -> GradientPaint {
    var result = GradientPaint();
    result.color_a = paint.a;
    result.color_b = paint.b;
    result.color_p1 = paint.c.xy;
    result.color_p2 = paint.c.zw;

    return result;
}

struct SampledPaint {
    color_tint: vec4f,
    color_uvwh: vec4f,
    alpha_uvwh: vec4f,
}

struct GradientPaint {
    color_a: vec4f,
    color_b: vec4f,
    color_p1: vec2f,
    color_p2: vec2f,
}
