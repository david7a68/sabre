use crate::ui::StyleClass;
use crate::ui::UiBuilder;
use crate::ui::style::StateFlags;

use super::impl_container;
use super::macros::forward_properties;

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
