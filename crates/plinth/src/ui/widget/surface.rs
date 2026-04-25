use crate::ui::StyleClass;
use crate::ui::UiBuilder;
use crate::ui::style::StateFlags;

use super::macros::forward_properties;
use super::macros::impl_container;

pub struct Surface<'a> {
    builder: UiBuilder<'a>,
}

impl<'a> Surface<'a> {
    pub fn new(builder: &'a mut UiBuilder<'_>) -> Self {
        let mut builder = builder.child();
        builder.apply_style(StyleClass::Surface, StateFlags::NORMAL);
        Self { builder }
    }

    forward_properties!(color, width, height, size, padding);
}

impl_container!(Surface<'a>);
