use glamour::Contains;
use winit::keyboard::KeyCode;
use winit::keyboard::PhysicalKey;

use crate::ui::Atom;
use crate::ui::AxisAnchor;
use crate::ui::LayoutDirection;
use crate::ui::OverlayPosition;
use crate::ui::Position;
use crate::ui::Size;
use crate::ui::StyleClass;
use crate::ui::UiBuilder;
use crate::ui::context::LayoutContent;
use crate::ui::style::CornerRadii;
use crate::ui::style::StateFlags;

use super::ClickBehavior;
use super::Interaction;

pub struct Dropdown<'a> {
    builder: Option<UiBuilder<'a>>,
    interaction: Interaction,
    is_open: bool,
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

        let overlay_id = root.id.then((id, "overlay"));
        let was_open = root.context.focused_widget == Some(overlay_id);

        let mut button = root.named_child((id, "trigger"));
        let pointer_over_trigger = button
            .prev_state()
            .map(|state| state.placement.contains(&button.input().pointer))
            .unwrap_or(false);

        let (interaction, state) = Interaction::compute(
            &button,
            ClickBehavior::OnPress,
            StateFlags::HOVERED | StateFlags::PRESSED,
        );

        button.apply_style(StyleClass::Button, state);
        button.set_active(state.contains(StateFlags::PRESSED));
        button.text(trigger_label, None);

        let trigger_width = button.prev_state().map(|state| state.placement.width());

        let is_open = if interaction.is_activated {
            !was_open
        } else {
            was_open
        };

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
        } else if was_open {
            // Clear focus when closing the dropdown to avoid keeping focus on a
            // now-nonexistent widget.
            root.context.focused_widget = None;
        }

        let mut dismiss_activated = false;
        let mut overlay_hovered = false;

        let mut overlay = if is_open {
            let dismiss_layer = root.layer.saturating_add(2);
            let dismiss_id = root.id.then((id, "dismiss"));
            let dismiss_index = root.context.ui_tree.add(
                Some(root.index),
                Atom {
                    width: Size::Fixed(root.input.window_size.width),
                    height: Size::Fixed(root.input.window_size.height),
                    position: Position::Absolute { x: 0.0, y: 0.0 },
                    z_layer: dismiss_layer,
                    is_modal: true,
                    ..Default::default()
                },
                (LayoutContent::None, Some(dismiss_id)),
            );

            root.num_child_widgets += 1;

            let dismiss_builder = UiBuilder {
                theme: root.theme,
                input: root.input,
                context: root.context,
                clipboard: root.clipboard,
                format_buffer: root.format_buffer,
                text_context: root.text_context,
                text_layouts: root.text_layouts,
                id: dismiss_id,
                index: dismiss_index,
                style_id: root.style_id,
                state: root.state,
                num_child_widgets: 0,
                is_modal: true,
                layer: dismiss_layer,
                text_overflow: root.text_overflow,
            };

            let (dismiss_interaction, _) = Interaction::compute(
                &dismiss_builder,
                ClickBehavior::OnPress,
                StateFlags::HOVERED | StateFlags::PRESSED,
            );
            dismiss_activated = dismiss_interaction.is_activated;

            let child_layer = root.layer.saturating_add(2);
            let child_id = root.id.then((id, "overlay"));
            let child_index = root.context.ui_tree.add(
                Some(root.index),
                Atom {
                    position: Position::OutOfFlow(OverlayPosition {
                        parent_x: AxisAnchor::Start,
                        parent_y: AxisAnchor::End,
                        self_x: AxisAnchor::Start,
                        self_y: AxisAnchor::Start,
                        offset: (0.0, 0.0),
                        flip_x: false,
                        flip_y: true,
                    }),
                    z_layer: child_layer,
                    is_modal: true,
                    ..Default::default()
                },
                (LayoutContent::None, Some(child_id)),
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
                text_overflow: root.text_overflow,
            })
        } else {
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
        }

        let (highlighted_index, keyboard_active) = overlay.as_ref()
            .and_then(|o| o.prev_state())
            .and_then(|s| s.custom_data::<[u32; 2]>())
            .map(|[idx, flag]| (Some(idx), flag != 0))
            .unwrap_or((None, false));

        Self {
            builder: overlay,
            interaction,
            is_open,
            num_items: 0,
            highlighted_index,
            keyboard_active,
            selected_index: None,
            close_requested: dismiss_activated
                && !overlay_hovered
                && !interaction.is_hovered
                && !pointer_over_trigger,
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
        if !self.is_open {
            return (self.selected_index.map(|i| i as usize), self.interaction);
        }

        self.handle_keyboard_input();

        if let (Some(builder), Some(idx)) = (self.builder.as_mut(), self.highlighted_index) {
            builder.context.state_mut(builder.id).set_custom_data([idx, self.keyboard_active as u32]);
        }

        if self.close_requested {
            if let Some(builder) = self.builder.as_mut() {
                builder.release_focus();
            }

            self.is_open = false;
            return (self.selected_index.map(|i| i as usize), self.interaction);
        }

        if let Some(builder) = self.builder.as_mut() {
            builder.request_focus();
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
            StateFlags::HOVERED | StateFlags::PRESSED | StateFlags::SELECTED,
        );

        if item_interaction.is_activated {
            self.selected_index = Some(item_index);
            self.close_requested = true;
        }

        if item_state.contains(StateFlags::HOVERED) {
            if !self.keyboard_active || mouse_moved {
                self.highlighted_index = Some(item_index);
                self.keyboard_active = false;
            }
        }

        let mut effective_state = item_state;
        if self.keyboard_active {
            effective_state.remove(StateFlags::HOVERED);
        }
        if Some(item_index) == self.highlighted_index {
            effective_state |= StateFlags::HOVERED;
        }

        let button_padding = item.theme().get(StyleClass::Button).padding.get(effective_state);

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

        let input = builder.input();

        for event in input.keyboard_events.iter() {
            if !event.state.is_pressed() {
                continue;
            }

            match event.key {
                PhysicalKey::Code(KeyCode::ArrowUp) => {
                    self.keyboard_active = true;
                    if let Some(idx) = self.highlighted_index {
                        if idx > 0 {
                            self.highlighted_index = Some(idx - 1);
                        }
                    } else if self.num_items > 0 {
                        self.highlighted_index = Some(self.num_items - 1);
                    }
                }
                PhysicalKey::Code(KeyCode::ArrowDown) => {
                    self.keyboard_active = true;
                    if let Some(idx) = self.highlighted_index {
                        if idx < self.num_items - 1 {
                            self.highlighted_index = Some(idx + 1);
                        }
                    } else if self.num_items > 0 {
                        self.highlighted_index = Some(0);
                    }
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
