use crate::graphics::Color;
use crate::ui::LayoutDirection;
use crate::ui::Size;
use crate::ui::UiBuilder;
use crate::ui::style::StateFlags;

use super::ClickBehavior;
use super::Interaction;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SplitAxis {
    Horizontal,
    Vertical,
}

#[derive(Clone, Copy, Debug)]
pub struct SplitPaneState {
    pub fraction: f32,
    pub is_resizing: bool,
}

impl SplitPaneState {
    pub fn new(fraction: f32) -> Self {
        Self {
            fraction,
            is_resizing: false,
        }
    }

    pub fn show(
        &mut self,
        builder: &mut UiBuilder<'_>,
        config: SplitPaneConfig,
        first: impl FnOnce(&mut UiBuilder<'_>),
        second: impl FnOnce(&mut UiBuilder<'_>),
    ) {
        render_split_pane_parts(
            builder,
            &mut self.fraction,
            &mut self.is_resizing,
            config,
            first,
            second,
        );
    }
}

impl Default for SplitPaneState {
    fn default() -> Self {
        Self::new(0.5)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct SplitPaneConfig {
    pub axis: SplitAxis,
    pub min_first: f32,
    pub min_second: f32,
    pub resizable: bool,
    pub divider_thickness: f32,
    pub divider_color: Color,
    pub divider_hover_color: Color,
}

impl SplitPaneConfig {
    pub fn horizontal() -> Self {
        Self {
            axis: SplitAxis::Horizontal,
            ..Self::default()
        }
    }

    pub fn vertical() -> Self {
        Self {
            axis: SplitAxis::Vertical,
            ..Self::default()
        }
    }

    pub fn with_min_first(mut self, min_first: f32) -> Self {
        self.min_first = min_first;
        self
    }

    pub fn with_min_second(mut self, min_second: f32) -> Self {
        self.min_second = min_second;
        self
    }

    pub fn with_resizable(mut self, resizable: bool) -> Self {
        self.resizable = resizable;
        self
    }

    pub fn with_divider_thickness(mut self, divider_thickness: f32) -> Self {
        self.divider_thickness = divider_thickness;
        self
    }
}

impl Default for SplitPaneConfig {
    fn default() -> Self {
        Self {
            axis: SplitAxis::Horizontal,
            min_first: 80.0,
            min_second: 80.0,
            resizable: true,
            divider_thickness: 6.0,
            divider_color: Color::srgb_nonlinear(0.18, 0.18, 0.18, 1.0),
            divider_hover_color: Color::srgb_nonlinear(0.35, 0.35, 0.35, 1.0),
        }
    }
}

pub(super) fn render_split_pane_parts(
    builder: &mut UiBuilder<'_>,
    fraction: &mut f32,
    is_resizing: &mut bool,
    config: SplitPaneConfig,
    first: impl FnOnce(&mut UiBuilder<'_>),
    second: impl FnOnce(&mut UiBuilder<'_>),
) {
    let axis = config.axis;
    let available = match axis {
        SplitAxis::Horizontal => builder.input().window_size.width,
        SplitAxis::Vertical => builder.input().window_size.height,
    };
    let delta = match axis {
        SplitAxis::Horizontal => builder.input().pointer.x - builder.input().prev_pointer.x,
        SplitAxis::Vertical => builder.input().pointer.y - builder.input().prev_pointer.y,
    };

    let usable = (available - config.divider_thickness).max(1.0);
    if config.resizable && *is_resizing {
        *fraction += delta / usable;
    }
    let min_fraction = config.min_first / usable;
    let max_fraction = 1.0 - config.min_second / usable;
    *fraction = if min_fraction <= max_fraction {
        fraction.clamp(min_fraction, max_fraction)
    } else {
        0.5
    };

    builder.child_direction(match axis {
        SplitAxis::Horizontal => LayoutDirection::Horizontal,
        SplitAxis::Vertical => LayoutDirection::Vertical,
    });

    let first_size = usable * *fraction;
    let second_size = usable - first_size;

    builder.with_named_child("first", |ui| {
        match axis {
            SplitAxis::Horizontal => {
                ui.width(first_size).height(Size::Grow);
            }
            SplitAxis::Vertical => {
                ui.width(Size::Grow).height(first_size);
            }
        };
        first(ui);
    });

    let mut divider = builder.named_child("divider");
    match axis {
        SplitAxis::Horizontal => {
            divider.width(config.divider_thickness).height(Size::Grow);
        }
        SplitAxis::Vertical => {
            divider.width(Size::Grow).height(config.divider_thickness);
        }
    };
    let (interaction, state_flags) = if config.resizable {
        Interaction::compute(
            &divider,
            ClickBehavior::OnPress,
            StateFlags::HOVERED | StateFlags::PRESSED,
        )
    } else {
        (
            Interaction {
                is_activated: false,
                is_hovered: false,
                is_focused: false,
            },
            StateFlags::NORMAL,
        )
    };
    let mouse_down = divider.input().mouse_state.is_left_down();
    if interaction.is_activated {
        *is_resizing = true;
    }
    if !mouse_down {
        *is_resizing = false;
    }
    divider.set_active(*is_resizing && mouse_down);
    divider.color(
        if state_flags.intersects(StateFlags::HOVERED | StateFlags::PRESSED) || *is_resizing {
            config.divider_hover_color
        } else {
            config.divider_color
        },
    );
    drop(divider);

    builder.with_named_child("second", |ui| {
        match axis {
            SplitAxis::Horizontal => {
                ui.width(second_size).height(Size::Grow);
            }
            SplitAxis::Vertical => {
                ui.width(Size::Grow).height(second_size);
            }
        };
        second(ui);
    });
}
