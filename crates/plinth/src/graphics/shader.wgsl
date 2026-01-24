struct DrawInfo {
    viewport_size: vec2<u32>
}

// Rectangle primitive with configurable paint (sampled texture or gradient)
struct Rect {
    point: vec2f,
    extent: vec2f,
    background: Paint,
    border_color: GradientPaint,
    // left, top, right, bottom
    border_width: vec4f,
    // top-left, top-right, bottom-left, bottom-right
    corner_radii: vec4f,
    control_flags: Bitflags,
    _padding0: u32,
    _padding1: u32,
    _padding2: u32,
}

struct VertexOutput {
    @builtin(position) frag_coord: vec4<f32>,
    @location(0) @interpolate(flat) rect_index: u32,
    @location(1) uv: vec2f,
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
    let vertex_corner = CORNER[vertex_index];
    let vertex_position = rect.point + EXTENT_LOOKUP[vertex_corner] * rect.extent;

    var out: VertexOutput;

    out.rect_index = rect_index;
    out.frag_coord = to_clip_coords(vertex_position);
    out.uv = EXTENT_LOOKUP[vertex_corner];

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

    let rect_center = rect.point + rect.extent * 0.5;
    let corner_radius = rect.corner_radii[corner_from_uv(in.uv)];

    let shape_distance = distance_from_rect(
        in.frag_coord.xy,
        rect_center,
        rect.extent * 0.5,
        corner_radius
    );

    // Anti-aliased edge: smoothstep over ~1 pixel
    let edge_alpha = 1.0 - smoothstep(-0.5, 0.5, shape_distance);
    if (edge_alpha <= 0.0) {
        discard;
    }

    var content_color: vec4f;
    if (is_gradient_paint(rect.control_flags)) {
        content_color = sample_gradient(as_gradient_paint(rect.background), in.uv);
    } else {
        // Sampled texture mode
        let sampled = as_sampled_paint(rect.background);
        
        let color_uv = sampled.color_uvwh.xy + sampled.color_uvwh.zw * in.uv;
        let alpha_uv = sampled.alpha_uvwh.xy + sampled.alpha_uvwh.zw * in.uv;

        if (is_nearest_sampling(rect.control_flags)) {
            content_color = sampled.color_tint * textureSample(color_texture, nearest_sampler, color_uv);
            content_color.a *= textureSample(alpha_texture, nearest_sampler, alpha_uv).r;
        } else {
            content_color = sampled.color_tint * textureSample(color_texture, basic_sampler, color_uv);
            content_color.a *= textureSample(alpha_texture, basic_sampler, alpha_uv).r;
        }
    }

    // Skip border calculation if no border
    let has_border = any(rect.border_width != vec4f(0.0));
    if (has_border) {
        let inner_point = rect.point + vec2f(rect.border_width.x, rect.border_width.y);
        let inner_extent = rect.extent - vec2f(rect.border_width.x + rect.border_width.z, rect.border_width.y + rect.border_width.w);
        let inner_center = inner_point + inner_extent * 0.5;
        
        let inner_corner_radius = corner_radius - max(
            max(rect.border_width.x, rect.border_width.y),
            max(rect.border_width.z, rect.border_width.w)
        );
        
        let border_distance = distance_from_rect(
            in.frag_coord.xy,
            inner_center,
            inner_extent * 0.5,
            inner_corner_radius
        );

        // Only blend if we're near or inside the border region
        // border_distance > 0 means we're in the border (outside inner rect)
        // We need AA on the inner edge, so check > -0.5
        if (border_distance > -0.5) {
            let border_blend = smoothstep(-0.5, 0.5, border_distance);
            let border_color = sample_gradient(rect.border_color, in.uv);
            content_color = mix(content_color, border_color, border_blend);
        }
    }

    content_color.a *= edge_alpha;
    return content_color;
}

const TOP_LEFT: u32 = 0u;
const TOP_RIGHT: u32 = 1u;
const BOTTOM_LEFT: u32 = 2u;
const BOTTOM_RIGHT: u32 = 3u;

/// Triangle layout for top-left origin (Y down):
/// 0 3---5
/// |\ \  |
/// | \ \ |
/// |  \ \|
/// 1---2 4
const CORNER: array<u32, 6> = array<u32, 6>(
    TOP_LEFT,
    BOTTOM_LEFT,
    BOTTOM_RIGHT,
    TOP_LEFT,
    BOTTOM_RIGHT,
    TOP_RIGHT,
);

const EXTENT_LOOKUP: array<vec2f, 4> = array<vec2f, 4>(
    vec2f(0.0, 0.0), // top-left
    vec2f(1.0, 0.0), // top-right
    vec2f(0.0, 1.0), // bottom-left
    vec2f(1.0, 1.0), // bottom-right
);

fn corner_from_uv(uv: vec2f) -> u32 {
    let is_right = uv.x >= 0.5;
    let is_bottom = uv.y >= 0.5;
    return select(
        select(TOP_LEFT, TOP_RIGHT, is_right),
        select(BOTTOM_LEFT, BOTTOM_RIGHT, is_right),
        is_bottom
    );
}

fn to_clip_coords(position: vec2f) -> vec4f {
    let x = position.x / f32(draw_info.viewport_size.x) * 2.0 - 1.0;
    let y = -(position.y / f32(draw_info.viewport_size.y) * 2.0 - 1.0);
    return vec4f(x, y, 0.0, 1.0);
}

fn distance_from_rect(point: vec2f, rect_center: vec2f, rect_half_extent: vec2f, corner_radius: f32) -> f32 {
    let local_pos = point - rect_center;
    let q = abs(local_pos) - rect_half_extent + vec2f(corner_radius, corner_radius);
    return length(max(q, vec2f(0.0, 0.0))) + min(max(q.x, q.y), 0.0) - corner_radius;
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

fn sample_gradient(gradient: GradientPaint, uv: vec2f) -> vec4f {
    let gradient_dir = gradient.color_p2 - gradient.color_p1;
    let gradient_len_sq = dot(gradient_dir, gradient_dir);
    
    var t: f32;
    if (gradient_len_sq < 0.0001) {
        t = 0.0;
    } else {
        t = clamp(dot(uv - gradient.color_p1, gradient_dir) / gradient_len_sq, 0.0, 1.0);
    }
    
    return mix(gradient.color_a, gradient.color_b, t);
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
