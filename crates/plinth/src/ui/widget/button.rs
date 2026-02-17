use crate::ui::StyleClass;
use crate::ui::UiBuilder;
use crate::ui::style::StateFlags;

use super::ClickBehavior;
use super::Interaction;
use super::impl_container;
use super::macros::forward_properties;

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
