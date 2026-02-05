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

        let prev_state = builder.prev_state();
        let input = builder.input();

        let (interaction, is_active) =
            Interaction::compute(prev_state, input, ClickBehavior::OnPress);

        let mut state = StateFlags::NORMAL;
        if interaction.is_hovered {
            state |= StateFlags::HOVERED;
        }
        if is_active {
            state |= StateFlags::PRESSED;
        }

        builder.set_active(is_active);
        builder.apply_style(StyleClass::Button, state);

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
