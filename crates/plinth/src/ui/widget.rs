use glamour::Contains;
use glamour::Rect;

use crate::ui::Pixels;
use crate::ui::text::TextLayoutId;

use super::Size;
use super::UiBuilder;
use super::style::StateFlags;

pub mod button;
pub mod label;
pub mod panel;
pub mod text_edit;

#[derive(Clone, Copy, Debug)]
pub struct Interaction {
    pub is_activated: bool,
    pub is_hovered: bool,
    pub is_focused: bool,
}

pub trait UiBuilderWidgetsExt {
    fn panel(&mut self) -> panel::Panel<'_>;

    fn text_button(&mut self, label: &str) -> Interaction;

    fn text_edit(&mut self, initial_text: &str, width: f32) -> text_edit::TextEdit<'_>;

    fn label(&mut self, text: &str) -> label::Label<'_>;
}

impl UiBuilderWidgetsExt for UiBuilder<'_> {
    fn panel(&mut self) -> panel::Panel<'_> {
        panel::Panel::new(self)
    }

    fn text_button(&mut self, label: &str) -> Interaction {
        button::Button::new(self, Some(label)).finish()
    }

    fn text_edit(&mut self, default_text: &str, width: f32) -> text_edit::TextEdit<'_> {
        text_edit::TextEdit::new(self, Size::Fixed(width)).default_text(default_text)
    }

    fn label(&mut self, text: &str) -> label::Label<'_> {
        label::Label::new(self, text)
    }
}

/// Controls when a click is registered.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ClickBehavior {
    /// Click fires immediately when mouse is pressed (more responsive).
    #[default]
    OnPress,
    /// Click fires when mouse is released while still hovered (standard UI behavior).
    OnRelease,
}

impl Interaction {
    /// Compute interaction state for a widget.
    ///
    /// Returns the interaction result and whether the widget is currently active (being pressed).
    pub fn compute(
        builder: &UiBuilder<'_>,
        behavior: ClickBehavior,
        interest: StateFlags,
    ) -> (Self, StateFlags) {
        let was_focused = builder.is_focused();
        let (was_active, is_hovered) = builder
            .prev_state()
            .map(|s| (s.was_active, s.placement.contains(&builder.input.pointer)))
            .unwrap_or_default();

        let is_left_down = builder.input.mouse_state.is_left_down();
        let just_pressed = is_left_down && !was_active;
        let just_released = !is_left_down && was_active;

        let is_activated = match behavior {
            ClickBehavior::OnPress => is_hovered && just_pressed,
            ClickBehavior::OnRelease => is_hovered && just_released,
        };

        let mut state = StateFlags::NORMAL;
        if is_hovered {
            state |= StateFlags::HOVERED & interest;
        }
        if is_hovered && is_left_down {
            state |= StateFlags::PRESSED & interest;
        }
        if is_activated || ((is_hovered || !just_pressed) && was_focused) {
            state |= StateFlags::FOCUSED & interest;
        }

        (
            Self {
                is_activated,
                is_hovered,
                is_focused: state.contains(StateFlags::FOCUSED),
            },
            state,
        )
    }
}

#[derive(Default)]
pub struct WidgetState {
    pub placement: Rect<Pixels>,
    /// Whether the widget was being actively pressed last frame
    pub was_active: bool,

    pub text_layout: Option<TextLayoutId>,
}
