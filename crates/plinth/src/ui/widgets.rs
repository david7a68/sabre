use core::f32;
use std::borrow::Cow;

use glamour::Contains;

use crate::graphics::Color;
use crate::graphics::text::TextStyle;
use crate::ui::Alignment;
use crate::ui::Padding;
use crate::ui::Response;
use crate::ui::Size;
use crate::ui::UiBuilder;
use crate::ui::Widget;

pub trait UiBuilderWidgetsExt {
    fn panel(
        &mut self,
        width: impl Into<Size>,
        height: impl Into<Size>,
        color: Option<Color>,
    ) -> Response;

    fn text_button<'a>(&mut self, label: impl Into<Cow<'a, str>>, style: &'a TextStyle)
    -> Response;
}

impl UiBuilderWidgetsExt for UiBuilder<'_> {
    fn panel(
        &mut self,
        width: impl Into<Size>,
        height: impl Into<Size>,
        color: Option<Color>,
    ) -> Response {
        Panel {
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

pub struct Panel {
    pub width: Size,
    pub height: Size,
    pub color: Option<Color>,
}

impl Widget for Panel {
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
    pub padding: Padding,
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
            padding: Padding {
                left: 10.0,
                right: 10.0,
                top: 5.0,
                bottom: 5.0,
            },
            width: Size::Fit {
                min: 20.0,
                max: f32::MAX,
            },
            height: Size::Fit {
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

    pub fn padding(mut self, padding: Padding) -> Self {
        self.padding = padding;
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
            .size(self.width, self.height)
            .padding(self.padding)
            .color(color);

        if let Some((label, style)) = self.label {
            widget.label(&label, style, None);
        }

        Response {
            is_hovered,
            is_clicked,
        }
    }
}
