use std::ops::Deref;
use std::ops::DerefMut;

use crate::ui::StyleClass;
use crate::ui::UiBuilder;
use crate::ui::style::StateFlags;

pub struct Panel<'a> {
    builder: UiBuilder<'a>,
}

impl<'a> Panel<'a> {
    pub fn new(builder: &'a mut UiBuilder<'_>) -> Self {
        let mut builder = builder.child();
        builder.apply_style(StyleClass::Panel, StateFlags::NORMAL);
        Self { builder }
    }
}

impl<'a> DerefMut for Panel<'a> {
    fn deref_mut(&mut self) -> &mut UiBuilder<'a> {
        &mut self.builder
    }
}

impl<'a> Deref for Panel<'a> {
    type Target = UiBuilder<'a>;

    fn deref(&self) -> &Self::Target {
        &self.builder
    }
}
