use glamour::Contains;
use glamour::Point2;

use crate::ui::AxisAnchor;
use crate::ui::LayoutDirection;
use crate::ui::OverlayPosition;
use crate::ui::Pixels;
use crate::ui::Position;
use crate::ui::Size;
use crate::ui::StyleClass;
use crate::ui::UiBuilder;
use crate::ui::style::StateFlags;

use super::ClickBehavior;
use super::Interaction;
use super::PointerButton;
use super::menu::MenuItem as ContextMenuItem;
use super::menu::MenuPopup;

pub struct ContextMenu<'a> {
    popup: MenuPopup<'a>,
}

impl<'a> ContextMenu<'a> {
    pub fn new(
        builder: &'a mut UiBuilder<'_>,
        id: &str,
        trigger: impl FnOnce(&mut UiBuilder<'_>),
    ) -> Self {
        Self::new_inner(builder, id, trigger)
    }

    fn new_inner(
        builder: &'a mut UiBuilder<'_>,
        id: &str,
        trigger: impl FnOnce(&mut UiBuilder<'_>),
    ) -> Self {
        let mut root = builder.named_child((id, "root"));
        root.child_direction(LayoutDirection::Vertical);
        root.child_spacing(0.0);
        trigger(&mut root);

        let root_id = root.id;
        let root_state = root
            .prev_state()
            .and_then(|s| s.custom_data::<RootState>())
            .unwrap_or_default();
        let was_open = root_state.is_open();
        let pointer = root.input().pointer;
        let pointer_over_trigger = root
            .prev_state()
            .is_some_and(|s| s.placement.contains(&pointer));
        let press_started_over_trigger = root
            .prev_state()
            .is_some_and(|s| s.placement.contains(&root.input().prev_pointer));

        let (mut interaction, state) = Interaction::compute_for_button(
            &root,
            PointerButton::Right,
            ClickBehavior::OnPress,
            StateFlags::HOVERED | StateFlags::PRESSED,
        );
        if !press_started_over_trigger {
            interaction.is_activated = false;
        }
        root.set_button_active(PointerButton::Right, state.contains(StateFlags::PRESSED));

        let is_open = was_open || interaction.is_activated;
        let mut anchor = if interaction.is_activated {
            root.input.pointer
        } else {
            root_state.anchor()
        };

        let mut dismiss_activated = false;
        let mut overlay_hovered = false;

        let mut overlay = if is_open {
            let dismiss = super::menu::dismiss_state(
                &mut root,
                id,
                &[PointerButton::Left, PointerButton::Right],
            );
            dismiss_activated = dismiss.any_activated();
            if dismiss.button_activated(PointerButton::Right) && pointer_over_trigger {
                anchor = root.input.pointer;
            }

            root.context.state_mut(root_id).set_custom_data(RootState {
                anchor_x_bits: anchor.x.to_bits(),
                anchor_y_bits: anchor.y.to_bits(),
            });

            let child_layer = root.layer.saturating_add(2);
            let anchor_id = root.id.then((id, "anchor"));
            let anchor_index = root.context.add_overlay_node(
                root.index,
                anchor_id,
                Position::Absolute {
                    x: anchor.x,
                    y: anchor.y,
                },
                child_layer,
                false,
            );
            root.num_child_widgets += 1;

            let child_id = anchor_id.then((id, "overlay"));
            let child_index = root.context.add_overlay_node(
                anchor_index,
                child_id,
                Position::OutOfFlow(OverlayPosition {
                    parent_x: AxisAnchor::Start,
                    parent_y: AxisAnchor::Start,
                    self_x: AxisAnchor::Start,
                    self_y: AxisAnchor::Start,
                    offset: (0.0, 0.0),
                    flip_x: true,
                    flip_y: true,
                }),
                child_layer,
                true,
            );

            Some(super::menu::add_overlay_builder(
                root,
                child_id,
                child_index,
                child_layer,
            ))
        } else {
            root.context
                .state_mut(root_id)
                .set_custom_data(RootState::default());
            None
        };

        if let Some(overlay) = overlay.as_mut() {
            overlay_hovered = super::menu::prepare_overlay(overlay, StyleClass::ContextMenu, None);
        }

        Self {
            popup: MenuPopup::new(
                overlay,
                root_id,
                interaction,
                dismiss_activated && !overlay_hovered && !pointer_over_trigger,
            ),
        }
    }

    pub fn width(&mut self, width: impl Into<Size>) -> &mut Self {
        self.popup.width(width);
        self
    }

    pub fn height(&mut self, height: impl Into<Size>) -> &mut Self {
        self.popup.height(height);
        self
    }

    pub fn size(&mut self, width: impl Into<Size>, height: impl Into<Size>) -> &mut Self {
        self.popup.size(width, height);
        self
    }

    pub fn padding(&mut self, padding: crate::ui::Padding) -> &mut Self {
        self.popup.padding(padding);
        self
    }

    pub fn item<T>(&mut self, callback: T) -> &mut Self
    where
        T: ContextMenuItem,
    {
        self.item_inner(&callback)
    }

    pub fn finish(self) -> (Option<usize>, Interaction) {
        self.popup.finish(|state| {
            if let Some(root) = state.custom_data_mut::<RootState>() {
                *root = RootState::default();
            }
        })
    }

    fn item_inner(&mut self, callback: &dyn ContextMenuItem) -> &mut Self {
        self.popup.item(StyleClass::ContextMenuItem, None, callback);
        self
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
struct RootState {
    anchor_x_bits: u32,
    anchor_y_bits: u32,
}

impl RootState {
    const CLOSED_ANCHOR_X: u32 = u32::MAX;

    fn is_open(self) -> bool {
        self.anchor_x_bits != Self::CLOSED_ANCHOR_X
    }

    fn anchor(self) -> Point2<Pixels> {
        Point2::new(
            f32::from_bits(self.anchor_x_bits),
            f32::from_bits(self.anchor_y_bits),
        )
    }
}

impl Default for RootState {
    fn default() -> Self {
        Self {
            anchor_x_bits: Self::CLOSED_ANCHOR_X,
            anchor_y_bits: 0,
        }
    }
}

unsafe impl bytemuck::Pod for RootState {}
unsafe impl bytemuck::Zeroable for RootState {}
