use crate::ui::StyleClass;
use crate::ui::UiBuilder;
use crate::ui::style::StateFlags;

use super::macros::forward_properties;
use super::macros::impl_container;

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
        let layout = style.resolve_layout_style(StateFlags::NORMAL);

        let mut child = builder.child();
        child.child_alignment(layout.child_major_alignment, layout.child_minor_alignment);
        child.child_spacing(layout.child_spacing);
        child.child_direction(layout.child_direction);

        Self { builder: child }
    }

    forward_properties!(width, height, size, padding);
}

impl_container!(Frame<'a>);
