use std::ops::Deref;
use std::ops::DerefMut;

use crate::ui::Alignment;
use crate::ui::LayoutDirection;
use crate::ui::Size;
use crate::ui::StyleClass;
use crate::ui::UiBuilder;
use crate::ui::style::StateFlags;

/// An invisible, non-interactive layout widget for grouping other widgets
/// together.
///
/// The container itself does not have any visual representation, but it can be
/// used to apply layout properties to a group of child widgets. For example,
/// you can use a container to arrange a group of buttons in a horizontal row
/// with spacing between them.
///
///
/// By default, the container will inherit the child layout properties from the
/// theme's Panel style, but you can override these properties using the builder
/// methods.
pub struct Frame<'a> {
    builder: UiBuilder<'a>,
}

impl<'a> Frame<'a> {
    pub fn new(builder: &'a mut UiBuilder<'_>) -> Self {
        let style = builder.theme().get(StyleClass::Surface);

        let major_alignment = style.child_major_alignment.get(StateFlags::NORMAL);
        let minor_alignment = style.child_minor_alignment.get(StateFlags::NORMAL);
        let spacing = style.child_spacing.get(StateFlags::NORMAL);
        let direction = style.child_direction.get(StateFlags::NORMAL);

        let mut child = builder.child();

        child.child_alignment(major_alignment, minor_alignment);
        child.child_spacing(spacing);
        child.child_direction(direction);

        Self { builder: child }
    }

    pub fn with_width(mut self, width: impl Into<Size>) -> Self {
        self.builder.width(width);
        self
    }

    pub fn with_height(mut self, height: impl Into<Size>) -> Self {
        self.builder.height(height);
        self
    }

    pub fn with_size(mut self, width: impl Into<Size>, height: impl Into<Size>) -> Self {
        self.builder.size(width, height);
        self
    }

    pub fn with_child_direction(mut self, direction: LayoutDirection) -> Self {
        self.builder.child_direction(direction);
        self
    }

    pub fn with_child_alignment(mut self, horizontal: Alignment, vertical: Alignment) -> Self {
        self.builder.child_alignment(horizontal, vertical);
        self
    }
}

impl<'a> DerefMut for Frame<'a> {
    fn deref_mut(&mut self) -> &mut UiBuilder<'a> {
        &mut self.builder
    }
}

impl<'a> Deref for Frame<'a> {
    type Target = UiBuilder<'a>;

    fn deref(&self) -> &Self::Target {
        &self.builder
    }
}
