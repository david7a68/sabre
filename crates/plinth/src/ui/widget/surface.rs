use std::ops::Deref;
use std::ops::DerefMut;

use crate::ui::Alignment;
use crate::ui::LayoutDirection;
use crate::ui::Size;
use crate::ui::StyleClass;
use crate::ui::UiBuilder;
use crate::ui::style::StateFlags;

pub struct Surface<'a> {
    builder: UiBuilder<'a>,
}

impl<'a> Surface<'a> {
    pub fn new(builder: &'a mut UiBuilder<'_>) -> Self {
        let mut builder = builder.child();
        builder.apply_style(StyleClass::Surface, StateFlags::NORMAL);
        Self { builder }
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

impl<'a> DerefMut for Surface<'a> {
    fn deref_mut(&mut self) -> &mut UiBuilder<'a> {
        &mut self.builder
    }
}

impl<'a> Deref for Surface<'a> {
    type Target = UiBuilder<'a>;

    fn deref(&self) -> &Self::Target {
        &self.builder
    }
}
