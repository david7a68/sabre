use crate::graphics::Color;
use crate::graphics::GradientPaint;
use crate::graphics::Paint;
use crate::graphics::Texture;
use crate::ui::Size;
use crate::ui::StyleClass;
use crate::ui::UiBuilder;
use crate::ui::style::BorderWidths;
use crate::ui::style::CornerRadii;
use crate::ui::style::StateFlags;

pub struct Image<'a> {
    builder: UiBuilder<'a>,

    texture: Texture,
    mask: Option<Texture>,

    border: Option<GradientPaint>,
    border_widths: Option<BorderWidths>,
    corner_radii: Option<CornerRadii>,
}

impl<'a> Image<'a> {
    pub fn new(builder: &'a mut UiBuilder<'_>, texture: &Texture) -> Self {
        let mut builder = builder.child();

        builder.apply_style(StyleClass::Image, StateFlags::NORMAL);

        builder.size(texture.size()[0] as f32, texture.size()[1] as f32);

        Self {
            builder,
            texture: texture.clone(),
            mask: None,
            border: None,
            border_widths: None,
            corner_radii: None,
        }
    }

    pub fn scale(&mut self, scale: f32) -> &mut Self {
        let size = self.texture.size();
        self.builder
            .size(size[0] as f32 * scale, size[1] as f32 * scale);
        self
    }

    pub fn with_scale(mut self, scale: f32) -> Self {
        self.scale(scale);
        self
    }

    pub fn width(&mut self, width: impl Into<Size>) -> &mut Self {
        self.builder.width(width);
        self
    }

    pub fn with_width(mut self, width: impl Into<Size>) -> Self {
        self.builder.width(width);
        self
    }

    pub fn height(&mut self, height: impl Into<Size>) -> &mut Self {
        self.builder.height(height);
        self
    }

    pub fn with_height(mut self, height: impl Into<Size>) -> Self {
        self.builder.height(height);
        self
    }

    pub fn size(&mut self, width: impl Into<Size>, height: impl Into<Size>) -> &mut Self {
        self.builder.size(width, height);
        self
    }

    pub fn with_size(mut self, width: impl Into<Size>, height: impl Into<Size>) -> Self {
        self.builder.size(width, height);
        self
    }

    pub fn mask(&mut self, mask: Texture) -> &mut Self {
        self.mask = Some(mask);
        self
    }

    pub fn with_mask(mut self, mask: Texture) -> Self {
        self.mask = Some(mask);
        self
    }

    pub fn border(&mut self, border: GradientPaint, widths: BorderWidths) -> &mut Self {
        self.border = Some(border);
        self.border_widths = Some(widths);
        self
    }

    pub fn with_border(mut self, border: GradientPaint, widths: BorderWidths) -> Self {
        self.border = Some(border);
        self.border_widths = Some(widths);
        self
    }

    pub fn corner_radii(&mut self, corner_radii: CornerRadii) -> &mut Self {
        self.corner_radii = Some(corner_radii);
        self
    }

    pub fn with_corner_radii(mut self, corner_radii: CornerRadii) -> Self {
        self.corner_radii = Some(corner_radii);
        self
    }

    pub fn finish(mut self) {
        self.builder.paint(
            Paint::Sampled {
                color_tint: Color::WHITE,
                color_texture: Some(self.texture),
                alpha_texture: self.mask.take(),
            },
            self.border
                .unwrap_or(GradientPaint::solid(Color::TRANSPARENT)),
            self.border_widths.unwrap_or_default(),
            self.corner_radii.unwrap_or_default(),
        );
    }
}
