use glamour::Contains;
use winit::keyboard::KeyCode;
use winit::keyboard::PhysicalKey;

use crate::ui::AxisAnchor;
use crate::ui::LayoutDirection;
use crate::ui::OverlayPosition;
use crate::ui::Position;
use crate::ui::Size;
use crate::ui::StyleClass;
use crate::ui::TextOverflow;
use crate::ui::UiBuilder;
use crate::ui::WidgetId;
use crate::ui::style::CornerRadii;
use crate::ui::style::StateFlags;

use super::ClickBehavior;
use super::Interaction;

pub struct Dropdown<'a> {
    builder: Option<UiBuilder<'a>>,
    root_id: WidgetId,
    interaction: Interaction,
    num_items: u32,
    highlighted_index: Option<u32>,
    keyboard_active: bool,
    selected_index: Option<u32>,
    close_requested: bool,
}

impl<'a> Dropdown<'a> {
    pub fn new(builder: &'a mut UiBuilder<'_>, id: &str, trigger_label: &str) -> Self {
        let mut root = builder.named_child((id, "root"));
        root.child_direction(LayoutDirection::Vertical);
        root.child_spacing(0.0);

        let root_id = root.id;

        let root_state = root
            .prev_state()
            .and_then(|s| s.custom_data::<RootState>())
            .unwrap_or_default();
        let was_open = root_state.is_open != 0;
        // First open before the button has ever rendered: trigger_width_bits is 0,
        // overlay sizes to its content for that frame, then catches up next frame.
        let trigger_width = match root_state.trigger_width_bits {
            0 => None,
            bits => Some(f32::from_bits(bits)),
        };

        let mut button = root.named_child((id, "trigger"));
        let pointer = button.input().pointer;
        let pointer_over_trigger = button
            .prev_state()
            .is_some_and(|s| s.placement.contains(&pointer));

        let (interaction, state) = Interaction::compute(
            &button,
            ClickBehavior::OnPress,
            StateFlags::HOVERED | StateFlags::PRESSED,
        );

        button.apply_style(StyleClass::Button, state);
        button.set_active(state.contains(StateFlags::PRESSED));
        button.text(trigger_label, None);

        let trigger_width_bits = button
            .prev_state()
            .map(|s| f32::to_bits(s.placement.width()))
            .unwrap_or(root_state.trigger_width_bits);

        let is_open = was_open ^ interaction.is_activated;

        if is_open {
            let button_style = button.theme().get(StyleClass::Button);
            let radii = button_style.corner_radii.get(state);

            button.paint(
                button_style.background.get(state),
                button_style.border.get(state),
                button_style.border_widths.get(state),
                CornerRadii {
                    top_left: radii.top_left,
                    top_right: radii.top_right,
                    bottom_left: 0.0,
                    bottom_right: 0.0,
                },
            );
        }

        let mut dismiss_activated = false;
        let mut overlay_hovered = false;

        let mut overlay = if is_open {
            // Layer layout when open:
            //   root.layer + 0  trigger button (base layer)
            //   root.layer + 2  dismiss + overlay panel + its items.
            //
            // Dismiss MUST sit at the same z_layer as the overlay, not one below it.
            // `input_block_layer` uses strict-less-than: a modal at layer N blocks
            // input to anything with layer < N. If dismiss were at +1 and the overlay
            // (also modal) at +2, dismiss would block itself. With both at +2 the
            // overlay's children (items) are not blocked, and the `!overlay_hovered`
            // guard on `close_requested` prevents clicks on items from also firing
            // the dismiss path.
            dismiss_activated = {
                let window_w = root.input.window_size.width;
                let window_h = root.input.window_size.height;
                let mut dismiss = root.modal_offset_child(
                    (id, "dismiss"),
                    Position::Absolute { x: 0.0, y: 0.0 },
                    2,
                );
                dismiss.size(window_w, window_h);
                let (i, _) = Interaction::compute(
                    &dismiss,
                    ClickBehavior::OnPress,
                    StateFlags::HOVERED | StateFlags::PRESSED,
                );
                i.is_activated
            };

            // Write root state before root is moved into the overlay builder.
            // (state_mut would borrow root, conflicting with the move into UiBuilder below.)
            root.context.state_mut(root_id).set_custom_data(RootState {
                is_open: 1,
                trigger_width_bits,
            });

            // Overlay panel. Must be constructed via struct literal so its fields are
            // moved out of root, giving the builder the outer 'a lifetime rather than
            // a lifetime tied to a reborrow of the local `root`. The Atom setup is
            // factored into UiContext::add_overlay_node to avoid duplication.
            let child_layer = root.layer.saturating_add(2);
            let child_id = root.id.then((id, "overlay"));
            let child_index = root.context.add_overlay_node(
                root.index,
                child_id,
                Position::OutOfFlow(OverlayPosition {
                    parent_x: AxisAnchor::Start,
                    parent_y: AxisAnchor::End,
                    self_x: AxisAnchor::Start,
                    self_y: AxisAnchor::Start,
                    offset: (0.0, 0.0),
                    flip_x: false,
                    flip_y: true,
                }),
                child_layer,
                true,
            );
            root.num_child_widgets += 1;

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
            root.context.state_mut(root_id).set_custom_data(RootState {
                is_open: 0,
                trigger_width_bits,
            });
            None
        };

        if let Some(overlay) = overlay.as_mut() {
            let (overlay_interaction, overlay_state) = Interaction::compute(
                overlay,
                ClickBehavior::OnPress,
                StateFlags::HOVERED | StateFlags::PRESSED,
            );

            overlay_hovered = overlay_interaction.is_hovered;

            overlay.apply_style(StyleClass::DropdownMenu, overlay_state);
            if let Some(width) = trigger_width {
                overlay.width(Size::Fixed(width));
            }
            overlay.child_spacing(0.0);
            overlay.set_active(overlay_state.contains(StateFlags::PRESSED));
            overlay.child_direction(LayoutDirection::Vertical);
            // Claim keyboard focus so handle_keyboard_input only fires for this
            // dropdown and not for any other widget reading keyboard_events.
            overlay.request_focus();
        }

        let prev_overlay_state = overlay
            .as_ref()
            .and_then(|o| o.prev_state())
            .and_then(|s| s.custom_data::<OverlayState>())
            .unwrap_or_default();
        let highlighted_index = prev_overlay_state.highlighted_index();
        let keyboard_active = prev_overlay_state.keyboard_active != 0;

        Self {
            builder: overlay,
            root_id,
            interaction,
            num_items: 0,
            highlighted_index,
            keyboard_active,
            selected_index: None,
            close_requested: dismiss_activated && !overlay_hovered && !pointer_over_trigger,
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
        T: DropdownItem,
    {
        self.item_inner(&callback)
    }

    pub fn finish(mut self) -> (Option<usize>, Interaction) {
        if self.builder.is_none() {
            return (self.selected_index.map(|i| i as usize), self.interaction);
        }

        self.handle_keyboard_input();

        if let Some(builder) = self.builder.as_mut() {
            if self.close_requested {
                // Reset overlay state so the next open starts fresh, and flip root
                // back to closed so was_open is false on the next frame.
                builder
                    .context
                    .state_mut(builder.id)
                    .set_custom_data(OverlayState::default());
                if let Some(s) = builder
                    .context
                    .state_mut(self.root_id)
                    .custom_data_mut::<RootState>()
                {
                    s.is_open = 0;
                }
                builder.release_focus();
            } else {
                // Always write so a fresh open never reads stale data.
                builder
                    .context
                    .state_mut(builder.id)
                    .set_custom_data(OverlayState {
                        highlighted: self.highlighted_index.unwrap_or(OverlayState::NO_HIGHLIGHT),
                        keyboard_active: self.keyboard_active as u32,
                    });
            }
        }

        (self.selected_index.map(|i| i as usize), self.interaction)
    }

    fn item_inner(&mut self, callback: &dyn DropdownItem) -> &mut Self {
        let item_index = self.num_items;
        let Some(builder) = self.builder.as_mut() else {
            return self;
        };

        let mouse_moved = {
            let input = builder.input();
            input.pointer != input.prev_pointer
        };

        let mut item = builder.child();

        let (item_interaction, item_state) = Interaction::compute(
            &item,
            ClickBehavior::OnPress,
            StateFlags::HOVERED | StateFlags::PRESSED,
        );

        if item_interaction.is_activated {
            self.selected_index = Some(item_index);
            self.close_requested = true;
        }

        if item_state.contains(StateFlags::HOVERED) && (!self.keyboard_active || mouse_moved) {
            self.highlighted_index = Some(item_index);
            self.keyboard_active = false;
        }

        let mut effective_state = item_state;
        if self.keyboard_active {
            effective_state.remove(StateFlags::HOVERED);
        }
        if Some(item_index) == self.highlighted_index {
            effective_state |= StateFlags::HOVERED;
        }

        // Use the Button style's padding for item content so text alignment matches
        // the trigger label, giving the open menu a visually consistent inset.
        let button_padding = item
            .theme()
            .get(StyleClass::Button)
            .padding
            .get(effective_state);

        item.apply_style(StyleClass::DropdownItem, effective_state);
        item.set_clip_children(true);
        item.set_active(item_state.contains(StateFlags::PRESSED));
        item.padding(button_padding);

        callback.build(&mut item);

        self.num_items += 1;
        self
    }

    fn handle_keyboard_input(&mut self) {
        let Some(builder) = self.builder.as_ref() else {
            return;
        };

        if !builder.is_focused() {
            return;
        }

        let input = builder.input();

        for event in input.keyboard_events.iter() {
            if !event.state.is_pressed() {
                continue;
            }

            match event.key {
                PhysicalKey::Code(KeyCode::ArrowUp) if self.num_items > 0 => {
                    self.keyboard_active = true;
                    let next = match self.highlighted_index {
                        Some(idx) => idx.saturating_sub(1),
                        None => self.num_items - 1,
                    };
                    self.highlighted_index = Some(next);
                }
                PhysicalKey::Code(KeyCode::ArrowDown) if self.num_items > 0 => {
                    self.keyboard_active = true;
                    let next = match self.highlighted_index {
                        Some(idx) => (idx + 1).min(self.num_items - 1),
                        None => 0,
                    };
                    self.highlighted_index = Some(next);
                }
                PhysicalKey::Code(KeyCode::Enter) => {
                    if let Some(idx) = self.highlighted_index {
                        self.selected_index = Some(idx);
                        self.close_requested = true;
                    }
                }
                PhysicalKey::Code(KeyCode::Escape) => {
                    self.close_requested = true;
                }
                _ => {}
            }
        }
    }
}

pub trait DropdownItem {
    fn build(&self, builder: &mut UiBuilder);
}

impl DropdownItem for &dyn DropdownItem {
    fn build(&self, builder: &mut UiBuilder) {
        (**self).build(builder);
    }
}

impl DropdownItem for &str {
    fn build(&self, builder: &mut UiBuilder) {
        builder.text(self, Size::default());
    }
}

impl DropdownItem for String {
    fn build(&self, builder: &mut UiBuilder) {
        builder.text(self.as_str(), Size::default());
    }
}

impl<F> DropdownItem for F
where
    F: Fn(&mut UiBuilder),
{
    fn build(&self, builder: &mut UiBuilder) {
        self(builder);
    }
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct RootState {
    is_open: u32,
    /// `f32::to_bits` of the trigger width; 0 means not yet measured.
    trigger_width_bits: u32,
}

unsafe impl bytemuck::Pod for RootState {}
unsafe impl bytemuck::Zeroable for RootState {}

#[repr(C)]
#[derive(Clone, Copy)]
struct OverlayState {
    highlighted: u32,
    keyboard_active: u32,
}

impl OverlayState {
    const NO_HIGHLIGHT: u32 = u32::MAX;

    fn highlighted_index(self) -> Option<u32> {
        (self.highlighted != Self::NO_HIGHLIGHT).then_some(self.highlighted)
    }
}

impl Default for OverlayState {
    fn default() -> Self {
        Self {
            highlighted: Self::NO_HIGHLIGHT,
            keyboard_active: 0,
        }
    }
}

unsafe impl bytemuck::Pod for OverlayState {}
unsafe impl bytemuck::Zeroable for OverlayState {}
