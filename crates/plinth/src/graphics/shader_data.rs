use bytemuck::Pod;
use bytemuck::Zeroable;

use crate::graphics::Color;

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct DrawUniforms {
    pub viewport_size: [u32; 2],
}

#[repr(C, align(16))]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub(crate) struct GpuPrimitive {
    pub point: [f32; 2],
    pub extent: [f32; 2],
    pub background: GpuPaint,
    pub border_color: GpuPaint,
    // left, top, right, bottom
    pub border_width: [f32; 4],
    // top-left, top-right, bottom-left, bottom-right
    pub corner_radii: [f32; 4],
    pub control_flags: PrimitiveRenderFlags,
    pub clip_idx: u32,
    pub _padding1: u32,
    pub _padding2: u32,
}

/// A union type representing either a sampled texture paint or a gradient paint.
/// The interpretation depends on the `USE_GRADIENT_PAINT` flag in `PrimitiveRenderFlags`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub struct GpuPaint {
    pub a: [f32; 4],
    pub b: [f32; 4],
    pub c: [f32; 4],
}

impl GpuPaint {
    /// Create a sampled texture paint.
    pub fn sampled(color_tint: Color, color_uvwh: [f32; 4], alpha_uvwh: [f32; 4]) -> Self {
        Self {
            a: color_tint.into(),
            b: color_uvwh,
            c: alpha_uvwh,
        }
    }

    /// Create a gradient paint.
    /// `p1` and `p2` are normalized coordinates (0.0-1.0) within the rect.
    pub fn gradient(color_a: Color, color_b: Color, p1: [f32; 2], p2: [f32; 2]) -> Self {
        Self {
            a: color_a.into(),
            b: color_b.into(),
            c: [p1[0], p1[1], p2[0], p2[1]],
        }
    }
}

bitflags::bitflags! {
    #[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
    #[repr(transparent)]
    pub struct PrimitiveRenderFlags: u32 {
        const USE_NEAREST_SAMPLING = 1;
        const USE_GRADIENT_PAINT = 2;
    }
}

#[repr(C, align(16))]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub(crate) struct GpuClip {
    pub point: [f32; 2],
    pub extent: [f32; 2],
    // left, top, right, bottom
    pub fade: [f32; 4],
}
