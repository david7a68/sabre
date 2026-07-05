use glamour::Point2;

use crate::ui::AxisAnchor;
use crate::ui::LayoutDirection;
use crate::ui::OverlayPosition;
use crate::ui::Pixels;
use crate::ui::Position;
use crate::ui::Size;
use crate::ui::StyleClass;
use crate::ui::TextOverflow;
use crate::ui::UiBuilder;
use crate::ui::WidgetId;
use crate::ui::style::StateFlags;

use super::ClickBehavior;
use super::Interaction;
use super::PointerButton;
use super::menu::MenuItem as ContextMenuItem;
use super::menu::MenuList;
use super::menu::MenuOverlayState;

pub struct ContextMenu<'a> {
    builder: Option<UiBuilder<'a>>,
    root_id: WidgetId,
    interaction: Interaction,
    menu: MenuList,
}

impl<'a> ContextMenu<'a> {
    pub fn new(
        builder: &'a mut UiBuilder<'_>,
        id: &str,
        trigger: impl FnOnce(&mut UiBuilder<'_>),
    ) -> Self {
        Self::new_inner(builder, id, Some(trigger), None)
    }

    pub fn at(builder: &'a mut UiBuilder<'_>, id: &str, position: Option<Point2<Pixels>>) -> Self {
        Self::new_inner(builder, id, None::<fn(&mut UiBuilder<'_>)>, position)
    }

    fn new_inner(
        builder: &'a mut UiBuilder<'_>,
        id: &str,
        trigger: Option<impl FnOnce(&mut UiBuilder<'_>)>,
        open_at: Option<Point2<Pixels>>,
    ) -> Self {
        let mut root = builder.named_child((id, "root"));
        root.child_direction(LayoutDirection::Vertical);
        root.child_spacing(0.0);

        if let Some(trigger) = trigger {
            trigger(&mut root);
        } else {
            root.size(0.0, 0.0);
        }

        let root_id = root.id;
        let root_state = root
            .prev_state()
            .and_then(|s| s.custom_data::<RootState>())
            .unwrap_or_default();
        let was_open = root_state.is_open();

        let (interaction, state) = Interaction::compute_for_button(
            &root,
            PointerButton::Right,
            ClickBehavior::OnPress,
            StateFlags::HOVERED | StateFlags::PRESSED,
        );
        root.set_button_active(PointerButton::Right, state.contains(StateFlags::PRESSED));

        let is_open = was_open || interaction.is_activated || open_at.is_some();
        let anchor = if let Some(position) = open_at {
            position
        } else if interaction.is_activated {
            root.input.pointer
        } else {
            root_state.anchor()
        };

        let mut dismiss_activated = false;
        let mut overlay_hovered = false;

        let mut overlay = if is_open {
            dismiss_activated = {
                let window_w = root.input.window_size.width;
                let window_h = root.input.window_size.height;

                let mut left_dismiss = root.modal_offset_child(
                    (id, "dismiss", PointerButton::Left),
                    Position::Absolute { x: 0.0, y: 0.0 },
                    2,
                );
                left_dismiss.size(window_w, window_h);
                let (left_interaction, left_state) = Interaction::compute(
                    &left_dismiss,
                    ClickBehavior::OnPress,
                    StateFlags::HOVERED | StateFlags::PRESSED,
                );
                left_dismiss.set_active(left_state.contains(StateFlags::PRESSED));

                let mut right_dismiss = root.modal_offset_child(
                    (id, "dismiss", PointerButton::Right),
                    Position::Absolute { x: 0.0, y: 0.0 },
                    2,
                );
                right_dismiss.size(window_w, window_h);
                let (right_interaction, right_state) = Interaction::compute_for_button(
                    &right_dismiss,
                    PointerButton::Right,
                    ClickBehavior::OnPress,
                    StateFlags::HOVERED | StateFlags::PRESSED,
                );
                right_dismiss.set_button_active(
                    PointerButton::Right,
                    right_state.contains(StateFlags::PRESSED),
                );

                left_interaction.is_activated || right_interaction.is_activated
            };

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

            Some(UiBuilder {
                theme: root.theme,
                input: root.input,
                context: root.context,
                clipboard: root.clipboard,
                format_buffer: root.format_buffer,
                text_context: root.text_context,
                text_layouts: root.text_layouts,
                id: child_id,
                index: child_index,
                style_id: root.style_id,
                state: root.state,
                num_child_widgets: 0,
                is_modal: true,
                layer: child_layer,
                text_overflow: TextOverflow::Clip,
            })
        } else {
            root.context
                .state_mut(root_id)
                .set_custom_data(RootState::default());
            None
        };

        if let Some(overlay) = overlay.as_mut() {
            let (overlay_interaction, overlay_state) = Interaction::compute(
                overlay,
                ClickBehavior::OnPress,
                StateFlags::HOVERED | StateFlags::PRESSED,
            );
            overlay_hovered = overlay_interaction.is_hovered;

            overlay.apply_style(StyleClass::ContextMenu, overlay_state);
            overlay.child_spacing(0.0);
            overlay.set_active(overlay_state.contains(StateFlags::PRESSED));
            overlay.child_direction(LayoutDirection::Vertical);
            overlay.request_focus();
        }

        let prev_overlay_state = overlay
            .as_ref()
            .and_then(|o| o.prev_state())
            .and_then(|s| s.custom_data::<MenuOverlayState>())
            .unwrap_or_default();
        let mut menu = MenuList::from_overlay_state(prev_overlay_state);
        menu.close_requested = dismiss_activated && !overlay_hovered;

        Self {
            builder: overlay,
            root_id,
            interaction,
            menu,
        }
    }

    pub fn width(&mut self, width: impl Into<Size>) -> &mut Self {
        if let Some(builder) = self.builder.as_mut() {
            builder.width(width);
        }
        self
    }

    pub fn height(&mut self, height: impl Into<Size>) -> &mut Self {
        if let Some(builder) = self.builder.as_mut() {
            builder.height(height);
        }
        self
    }

    pub fn size(&mut self, width: impl Into<Size>, height: impl Into<Size>) -> &mut Self {
        if let Some(builder) = self.builder.as_mut() {
            builder.size(width, height);
        }
        self
    }

    pub fn padding(&mut self, padding: crate::ui::Padding) -> &mut Self {
        if let Some(builder) = self.builder.as_mut() {
            builder.padding(padding);
        }
        self
    }

    pub fn item<T>(&mut self, callback: T) -> &mut Self
    where
        T: ContextMenuItem,
    {
        self.item_inner(&callback)
    }

    pub fn finish(mut self) -> (Option<usize>, Interaction) {
        if self.builder.is_none() {
            return (self.menu.selected_index(), self.interaction);
        }

        if let Some(builder) = self.builder.as_ref() {
            self.menu.handle_keyboard_input(builder);
        }

        if let Some(builder) = self.builder.as_mut() {
            if self.menu.close_requested {
                builder
                    .context
                    .state_mut(builder.id)
                    .set_custom_data(MenuOverlayState::default());
                if let Some(s) = builder
                    .context
                    .state_mut(self.root_id)
                    .custom_data_mut::<RootState>()
                {
                    *s = RootState::default();
                }
                builder.release_focus();
            } else {
                builder
                    .context
                    .state_mut(builder.id)
                    .set_custom_data(self.menu.overlay_state());
            }
        }

        (self.menu.selected_index(), self.interaction)
    }

    fn item_inner(&mut self, callback: &dyn ContextMenuItem) -> &mut Self {
        let Some(builder) = self.builder.as_mut() else {
            return self;
        };

        self.menu
            .item(builder, StyleClass::ContextMenuItem, None, callback);
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
