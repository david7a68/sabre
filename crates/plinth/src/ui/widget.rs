use glamour::Contains;
use glamour::Rect;

use crate::ui::Input;
use crate::ui::Pixels;

#[derive(Clone, Debug)]
pub struct Interaction {
    pub is_clicked: bool,
    pub is_hovered: bool,
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
        prev_state: Option<&WidgetState>,
        input: &Input,
        behavior: ClickBehavior,
    ) -> (Self, bool) {
        let is_hovered = prev_state
            .map(|s| s.placement.contains(&input.pointer))
            .unwrap_or(false);

        let was_active = prev_state.map(|s| s.was_active).unwrap_or(false);
        let is_left_down = input.mouse_state.is_left_down();
        let is_active = is_hovered && is_left_down;

        let is_clicked = match behavior {
            // Click on press: hovered, mouse just went down (wasn't active before)
            ClickBehavior::OnPress => is_hovered && is_left_down && !was_active,
            // Click on release: was active, mouse just released while still hovered
            ClickBehavior::OnRelease => is_hovered && was_active && !is_left_down,
        };

        (
            Self {
                is_clicked,
                is_hovered,
            },
            is_active,
        )
    }
}

pub struct WidgetState {
    pub placement: Rect<Pixels>,
    /// Whether the widget was being actively pressed last frame
    pub was_active: bool,
}
