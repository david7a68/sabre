use super::Color;
use super::Texture;

/// Defines how a primitive is painted - either with textures or a gradient.
#[derive(Debug, Clone, PartialEq)]
pub enum Paint {
    /// Paint using sampled textures with a color tint.
    Sampled {
        color_tint: Color,
        color_texture: Option<Texture>,
        alpha_texture: Option<Texture>,
    },
    /// Paint using a linear gradient between two colors.
    /// Points are in normalized coordinates (0.0-1.0) within the primitive bounds.
    Gradient {
        color_a: Color,
        color_b: Color,
        /// Start point of the gradient in normalized coordinates (0.0-1.0).
        start: [f32; 2],
        /// End point of the gradient in normalized coordinates (0.0-1.0).
        end: [f32; 2],
    },
}

impl Default for Paint {
    fn default() -> Self {
        Paint::Sampled {
            color_tint: Color::WHITE,
            color_texture: None,
            alpha_texture: None,
        }
    }
}

impl Paint {
    /// Create a solid color paint.
    pub fn solid(color: Color) -> Self {
        Paint::Sampled {
            color_tint: color,
            color_texture: None,
            alpha_texture: None,
        }
    }

    /// Create a textured paint with an optional color tint.
    pub fn textured(texture: Texture, tint: Color) -> Self {
        Paint::Sampled {
            color_tint: tint,
            color_texture: Some(texture),
            alpha_texture: None,
        }
    }

    /// Create a horizontal gradient from left to right.
    pub fn horizontal_gradient(left: Color, right: Color) -> Self {
        Paint::Gradient {
            color_a: left,
            color_b: right,
            start: [0.0, 0.5],
            end: [1.0, 0.5],
        }
    }

    /// Create a vertical gradient from top to bottom.
    pub fn vertical_gradient(top: Color, bottom: Color) -> Self {
        Paint::Gradient {
            color_a: top,
            color_b: bottom,
            start: [0.5, 0.0],
            end: [0.5, 1.0],
        }
    }

    /// Create a linear gradient with custom start and end points.
    /// Points are in normalized coordinates (0.0-1.0) within the primitive bounds.
    pub fn linear_gradient(color_a: Color, color_b: Color, start: [f32; 2], end: [f32; 2]) -> Self {
        Paint::Gradient {
            color_a,
            color_b,
            start,
            end,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct GradientPaint {
    pub color_a: Color,
    pub color_b: Color,
    pub start: [f32; 2],
    pub end: [f32; 2],
}

impl GradientPaint {
    /// Create a horizontal gradient from left to right.
    pub fn horizontal_gradient(left: Color, right: Color) -> Self {
        Self {
            color_a: left,
            color_b: right,
            start: [0.0, 0.5],
            end: [1.0, 0.5],
        }
    }

    /// Create a vertical gradient from top to bottom.
    pub fn vertical_gradient(top: Color, bottom: Color) -> Self {
        Self {
            color_a: top,
            color_b: bottom,
            start: [0.5, 0.0],
            end: [0.5, 1.0],
        }
    }

    /// Create a linear gradient with custom start and end points.
    /// Points are in normalized coordinates (0.0-1.0) within the primitive bounds.
    pub fn linear_gradient(color_a: Color, color_b: Color, start: [f32; 2], end: [f32; 2]) -> Self {
        Self {
            color_a,
            color_b,
            start,
            end,
        }
    }
}
