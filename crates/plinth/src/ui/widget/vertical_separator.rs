use crate::graphics::Color;
use crate::graphics::GradientPaint;
use crate::graphics::Paint;
use crate::ui::Padding;
use crate::ui::Size;
use crate::ui::StyleClass;
use crate::ui::UiBuilder;
use crate::ui::style::BorderWidths;
use crate::ui::style::CornerRadii;
use crate::ui::style::StateFlags;

pub struct VerticalSeparator<'a> {
    builder: UiBuilder<'a>,
}

impl<'a> VerticalSeparator<'a> {
    pub fn new(builder: &'a mut UiBuilder<'_>) -> Self {
        let mut builder = builder.child();
        builder.apply_style(StyleClass::VerticalSeparator, StateFlags::NORMAL);
        Self { builder }
    }

    pub fn paint(
        &mut self,
        paint: Paint,
        border: GradientPaint,
        border_width: BorderWidths,
        corner_radii: CornerRadii,
    ) -> &mut Self {
        self.builder
            .paint(paint, border, border_width, corner_radii);
        self
    }

    pub fn color(mut self, color: Color) -> Self {
        self.builder.color(color);
        self
    }

    pub fn thickness(mut self, thickness: f32) -> Self {
        self.builder.height(thickness);
        self
    }

    pub fn height(mut self, height: impl Into<Size>) -> Self {
        self.builder.height(height);
        self
    }

    pub fn padding(mut self, padding: Padding) -> Self {
        self.builder.padding(padding);
        self
    }
}
