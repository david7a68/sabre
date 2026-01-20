use std::ops::Deref;
use std::ops::DerefMut;

use crate::ui::ClickBehavior;
use crate::ui::Interaction;
use crate::ui::UiBuilder;

use super::style::StateFlags;
use super::theme::StyleClass;

pub trait UiBuilderWidgetsExt {
    fn panel(&mut self) -> Panel<'_>;

    fn text_button(&mut self, label: &str) -> Interaction;
}

impl UiBuilderWidgetsExt for UiBuilder<'_> {
    fn panel(&mut self) -> Panel<'_> {
        Panel::new(self)
    }

    fn text_button(&mut self, label: &str) -> Interaction {
        Button::new(self, Some(label)).finish()
    }
}

pub struct Panel<'a> {
    builder: UiBuilder<'a>,
}

impl<'a> Panel<'a> {
    pub fn new(builder: &'a mut UiBuilder<'_>) -> Self {
        let mut builder = builder.container();
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

pub struct Button<'a> {
    builder: UiBuilder<'a>,
}

impl Button<'_> {
    pub fn new<'a>(builder: &'a mut UiBuilder<'_>, label: Option<&str>) -> Button<'a> {
        let builder = if let Some(label) = label {
            let mut child = builder.named_child(label);
            child.label(label, None);
            child
        } else {
            builder.child()
        };

        Button { builder }
    }

    pub fn finish(mut self) -> Interaction {
        let prev_state = self.builder.prev_state();
        let input = self.builder.input();

        let (interaction, is_active) =
            Interaction::compute(prev_state, input, ClickBehavior::OnPress);

        self.builder.set_active(is_active);

        let mut state = StateFlags::NORMAL;
        if interaction.is_hovered {
            state |= StateFlags::HOVERED;
        }
        if is_active {
            state |= StateFlags::PRESSED;
        }

        self.builder.apply_style(StyleClass::Button, state);

        interaction
    }
}

impl<'a> DerefMut for Button<'a> {
    fn deref_mut(&mut self) -> &mut UiBuilder<'a> {
        &mut self.builder
    }
}

impl<'a> Deref for Button<'a> {
    type Target = UiBuilder<'a>;

    fn deref(&self) -> &Self::Target {
        &self.builder
    }
}
