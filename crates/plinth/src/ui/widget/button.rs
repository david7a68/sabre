use std::hash::Hash;

use crate::ui::StyleClass;
use crate::ui::UiBuilder;
use crate::ui::style::StateFlags;

use super::ClickBehavior;
use super::Interaction;
use super::macros::forward_properties;
use super::macros::impl_container;

pub struct Button<'a> {
    builder: UiBuilder<'a>,
    interaction: Interaction,
}

impl Button<'_> {
    pub fn new<'a>(builder: &'a mut UiBuilder<'_>, label: Option<&str>) -> Button<'a> {
        Self::from_builder(builder.child(), label)
    }

    pub fn with_id<'a>(
        builder: &'a mut UiBuilder<'_>,
        id: impl Hash,
        label: Option<&str>,
    ) -> Button<'a> {
        Self::from_builder(builder.named_child(id), label)
    }

    fn from_builder<'a>(mut builder: UiBuilder<'a>, label: Option<&str>) -> Button<'a> {
        let (interaction, state) = Interaction::compute(
            &builder,
            ClickBehavior::OnPress,
            StateFlags::HOVERED | StateFlags::PRESSED,
        );

        builder.apply_style(StyleClass::Button, state);
        builder.set_active(state.contains(StateFlags::PRESSED));

        if let Some(label_text) = label {
            builder.text(label_text, None);
        }

        Button {
            builder,
            interaction,
        }
    }

    forward_properties!(width, height, size, padding);

    pub fn finish(self) -> Interaction {
        self.interaction
    }
}

impl_container!(Button<'a>);
