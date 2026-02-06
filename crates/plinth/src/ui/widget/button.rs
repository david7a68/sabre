use std::ops::Deref;
use std::ops::DerefMut;

use crate::ui::StyleClass;
use crate::ui::UiBuilder;
use crate::ui::style::StateFlags;

use super::ClickBehavior;
use super::Interaction;

pub struct Button<'a> {
    builder: UiBuilder<'a>,
    interaction: Interaction,
}

impl Button<'_> {
    pub fn new<'a>(builder: &'a mut UiBuilder<'_>, label: Option<&str>) -> Button<'a> {
        let mut builder = match label {
            Some(label_text) => builder.named_child(label_text),
            None => builder.child(),
        };

        let (interaction, state) =
            Interaction::compute(&builder, ClickBehavior::OnPress, StateFlags::PRESSED);

        builder.apply_style(StyleClass::Button, state);
        if state.contains(StateFlags::PRESSED) {
            builder.set_active(true);
        }

        if let Some(label_text) = label {
            builder.set_text(label_text, None);
        }

        Button {
            builder,
            interaction,
        }
    }

    pub fn finish(self) -> Interaction {
        self.interaction
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
