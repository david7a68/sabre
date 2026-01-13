use std::borrow::Cow;

use glamour::Contains;

use crate::graphics::Color;
use crate::ui::Interaction;
use crate::ui::Padding;
use crate::ui::Size;
use crate::ui::UiBuilder;
use crate::ui::Widget;

use super::style::StateFlags;
use super::theme::StyleClass;

pub trait UiBuilderWidgetsExt {
    fn panel(
        &mut self,
        width: impl Into<Size>,
        height: impl Into<Size>,
        color: Option<Color>,
    ) -> Interaction;

    fn text_button<'a>(&mut self, label: impl Into<Cow<'a, str>>) -> Interaction;
}

impl UiBuilderWidgetsExt for UiBuilder<'_> {
    fn panel(
        &mut self,
        width: impl Into<Size>,
        height: impl Into<Size>,
        color: Option<Color>,
    ) -> Interaction {
        Panel {
            width: width.into(),
            height: height.into(),
            color,
        }
        .apply(self)
    }

    fn text_button<'a>(&mut self, label: impl Into<Cow<'a, str>>) -> Interaction {
        Button::new().label(label).apply(self)
    }
}

pub struct Panel {
    pub width: Size,
    pub height: Size,
    pub color: Option<Color>,
}

impl Widget for Panel {
    fn apply(self, context: &mut UiBuilder) -> Interaction {
        context.rect(self.width, self.height, self.color.unwrap_or_default());

        Interaction {
            is_clicked: false,
            is_hovered: false,
        }
    }
}

#[derive(Debug)]
pub struct Button<'a> {
    pub width: Option<Size>,
    pub height: Option<Size>,
    pub padding: Option<Padding>,
    pub label: Option<Cow<'a, str>>,
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
            padding: None,
            width: None,
            height: None,
        }
    }

    pub fn width(mut self, width: impl Into<Size>) -> Self {
        self.width = Some(width.into());
        self
    }

    pub fn height(mut self, height: impl Into<Size>) -> Self {
        self.height = Some(height.into());
        self
    }

    pub fn size(mut self, width: impl Into<Size>, height: impl Into<Size>) -> Self {
        self.width = Some(width.into());
        self.height = Some(height.into());
        self
    }

    pub fn padding(mut self, padding: Padding) -> Self {
        self.padding = Some(padding);
        self
    }

    pub fn label(mut self, label: impl Into<Cow<'a, str>>) -> Self {
        self.label = Some(label.into());
        self
    }
}

impl Widget for Button<'_> {
    fn apply(self, context: &mut UiBuilder) -> Interaction {
        let mut widget = if let Some(label) = &self.label {
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

        let state = if is_hovered {
            StateFlags::HOVERED
        } else {
            StateFlags::NORMAL
        };

        let style = widget.theme().get(StyleClass::Button);
        let color = style.background_color.get(state);
        let width = self.width.unwrap_or_else(|| style.width.get(state));
        let height = self.height.unwrap_or_else(|| style.height.get(state));
        let padding = self.padding.unwrap_or_else(|| style.padding.get(state));
        let major_align = style.child_major_alignment.get(state);
        let minor_align = style.child_minor_alignment.get(state);

        widget
            .child_alignment(major_align, minor_align)
            .size(width, height)
            .padding(padding)
            .color(color);

        if let Some(label) = self.label {
            widget.label(&label, None);
        }

        Interaction {
            is_hovered,
            is_clicked,
        }
    }
}
