use bytemuck::Pod;
use bytemuck::Zeroable;

// All colors are stored in linear sRGB space.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Color {
    pub const TRANSPARENT: Self = Self::linear(0.0, 0.0, 0.0, 0.0);
    pub const BLACK: Self = Self::linear(0.0, 0.0, 0.0, 1.0);
    pub const LIGHT_GRAY: Self = Self::linear(0.8, 0.8, 0.8, 1.0);
    pub const DARK_GRAY: Self = Self::linear(0.2, 0.2, 0.2, 1.0);
    pub const WHITE: Self = Self::linear(1.0, 1.0, 1.0, 1.0);
    pub const RED: Self = Self::linear(1.0, 0.0, 0.0, 1.0);
    pub const BLUE: Self = Self::linear(0.0, 0.0, 1.0, 1.0);
    pub const GREEN: Self = Self::linear(0.0, 1.0, 0.0, 1.0);

    /// Construct a color directly from linear sRGB components without any
    /// gamma conversion. Use this when the inputs are already linear.
    pub const fn linear(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }

    /// Construct a color from non-linear (gamma-encoded) sRGB components,
    /// converting them into the linear sRGB representation used internally.
    pub fn srgb_nonlinear(r: f32, g: f32, b: f32, a: f32) -> Self {
        let srgb =
            color::AlphaColor::<color::Srgb>::new([r, g, b, a]).convert::<color::LinearSrgb>();

        Self {
            r: srgb.components[0],
            g: srgb.components[1],
            b: srgb.components[2],
            a: srgb.components[3],
        }
    }

    /// Return a copy of this color with the alpha channel replaced.
    pub const fn with_alpha(mut self, a: f32) -> Self {
        self.a = a;
        self
    }

    /// Return a copy of this color with the alpha channel multiplied by `factor`.
    pub const fn mul_alpha(mut self, factor: f32) -> Self {
        self.a *= factor;
        self
    }
}

impl From<Option<Color>> for Color {
    fn from(color: Option<Color>) -> Self {
        color.unwrap_or_default()
    }
}

impl From<Color> for [f32; 4] {
    fn from(color: Color) -> Self {
        [color.r, color.g, color.b, color.a]
    }
}
