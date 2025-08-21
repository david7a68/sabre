use core::f32;
use std::borrow::Cow;

use glamour::Contains;
use graphics::Color;

use crate::Alignment;
use crate::Response;
use crate::Size;
use crate::UiBuilder;
use crate::Widget;
use crate::text::TextStyle;

pub trait UiBuilderWidgetsExt {
    fn plane(
        &mut self,
        width: impl Into<Size>,
        height: impl Into<Size>,
        color: Option<Color>,
    ) -> Response;

    fn text_button<'a>(&mut self, label: impl Into<Cow<'a, str>>, style: &'a TextStyle)
    -> Response;
}

impl UiBuilderWidgetsExt for UiBuilder<'_> {
    fn plane(
        &mut self,
        width: impl Into<Size>,
        height: impl Into<Size>,
        color: Option<Color>,
    ) -> Response {
        Plane {
            width: width.into(),
            height: height.into(),
            color,
        }
        .apply(self)
    }

    fn text_button<'a>(
        &mut self,
        label: impl Into<Cow<'a, str>>,
        style: &'a TextStyle,
    ) -> Response {
        Button::new().label(label, style).apply(self)
    }
}

pub struct Plane {
    pub width: Size,
    pub height: Size,
    pub color: Option<Color>,
}

impl Widget for Plane {
    fn apply(self, context: &mut UiBuilder) -> Response {
        context.rect(self.width, self.height, self.color.unwrap_or_default());

        Response {
            is_clicked: false,
            is_hovered: false,
        }
    }
}

#[derive(Debug)]
pub struct Button<'a> {
    pub width: Size,
    pub height: Size,
    pub label: Option<(Cow<'a, str>, &'a TextStyle)>,
}

impl Default for Button<'_> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'a> Button<'a> {
    pub fn new() -> Self {
        Self {
            label: None,
            width: Size::Flex {
                min: 20.0,
                max: f32::MAX,
            },
            height: Size::Flex {
                min: 10.0,
                max: f32::MAX,
            },
        }
    }

    pub fn text(mut self, width: impl Into<Size>) -> Self {
        self.width = width.into();
        self
    }

    pub fn height(mut self, height: impl Into<Size>) -> Self {
        self.height = height.into();
        self
    }

    pub fn size(mut self, width: impl Into<Size>, height: impl Into<Size>) -> Self {
        self.width = width.into();
        self.height = height.into();
        self
    }

    pub fn label(mut self, label: impl Into<Cow<'a, str>>, style: &'a TextStyle) -> Self {
        self.label = Some((label.into(), style));
        self
    }
}

impl Widget for Button<'_> {
    fn apply(self, context: &mut UiBuilder) -> Response {
        let mut widget = if let Some((label, _)) = &self.label {
            context.named_child(label)
        } else {
            context.child()
        };

        let is_hovered = if let Some(state) = widget.prev_state() {
            state.placement.contains(&widget.input().pointer)
        } else {
            false
        };

        let is_clicked = is_hovered && widget.input().mouse_state.is_left_down;

        let color = if is_hovered {
            Color::LIGHT_GRAY
        } else {
            Color::DARK_GRAY
        };

        widget
            .child_alignment(Alignment::Center, Alignment::Center)
            .width(self.width)
            .height(self.height)
            .color(color);

        if let Some((label, style)) = self.label {
            widget.label(&label, style, None, None);
        }

        Response {
            is_hovered,
            is_clicked,
        }
    }
}
