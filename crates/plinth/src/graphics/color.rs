use bytemuck::Pod;
use bytemuck::Zeroable;

// All colors are in linear sRGB space.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Color {
    pub const BLACK: Self = Self {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 1.0,
    };

    pub const LIGHT_GRAY: Self = Self {
        r: 0.8,
        g: 0.8,
        b: 0.8,
        a: 1.0,
    };

    pub const DARK_GRAY: Self = Self {
        r: 0.2,
        g: 0.2,
        b: 0.2,
        a: 1.0,
    };

    pub const WHITE: Self = Self {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a: 1.0,
    };

    pub const RED: Self = Self {
        r: 1.0,
        g: 0.0,
        b: 0.0,
        a: 1.0,
    };

    pub const BLUE: Self = Self {
        r: 0.0,
        g: 0.0,
        b: 1.0,
        a: 1.0,
    };

    pub const GREEN: Self = Self {
        r: 0.0,
        g: 1.0,
        b: 0.0,
        a: 1.0,
    };

    pub fn srgb(r: f32, g: f32, b: f32, a: f32) -> Self {
        let srgb =
            color::AlphaColor::<color::Srgb>::new([r, g, b, a]).convert::<color::LinearSrgb>();

        Self {
            r: srgb.components[0],
            g: srgb.components[1],
            b: srgb.components[2],
            a: srgb.components[3],
        }
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
