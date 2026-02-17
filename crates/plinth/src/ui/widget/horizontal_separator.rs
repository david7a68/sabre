use crate::graphics::GradientPaint;
use crate::graphics::Paint;
use crate::ui::StyleClass;
use crate::ui::UiBuilder;
use crate::ui::style::BorderWidths;
use crate::ui::style::CornerRadii;
use crate::ui::style::StateFlags;

use super::macros::forward_properties;

pub struct HorizontalSeparator<'a> {
    builder: UiBuilder<'a>,
}

impl<'a> HorizontalSeparator<'a> {
    pub fn new(builder: &'a mut UiBuilder<'_>) -> Self {
        let mut builder = builder.child();
        builder.apply_style(StyleClass::HorizontalSeparator, StateFlags::NORMAL);
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

    forward_properties!(color, width, padding);

    pub fn thickness(mut self, thickness: f32) -> Self {
        self.builder.height(thickness);
        self
    }
}
