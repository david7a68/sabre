use crate::ui::AxisAnchor;
use crate::ui::LayoutDirection;
use crate::ui::OverlayPosition;
use crate::ui::Size;
use crate::ui::StyleClass;
use crate::ui::UiBuilder;
use crate::ui::style::CornerRadii;
use crate::ui::style::StateFlags;
use glamour::Contains;

use super::ClickBehavior;
use super::Interaction;
use super::PointerButton;
use super::menu::MenuItem as DropdownItem;
use super::menu::MenuPopup;

pub struct Dropdown<'a> {
    popup: MenuPopup<'a>,
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
            dismiss_activated =
                super::menu::dismiss_state(&mut root, id, &[PointerButton::Left]).any_activated();

            root.context.state_mut(root_id).set_custom_data(RootState {
                is_open: 1,
                trigger_width_bits,
            });

            let child_layer = root.layer.saturating_add(2);
            let child_id = root.id.then((id, "overlay"));
            let child_index = super::menu::add_root_overlay(
                &mut root,
                child_id,
                OverlayPosition {
                    parent_x: AxisAnchor::Start,
                    parent_y: AxisAnchor::End,
                    self_x: AxisAnchor::Start,
                    self_y: AxisAnchor::Start,
                    offset: (0.0, 0.0),
                    flip_x: false,
                    flip_y: true,
                },
                child_layer,
            );

            Some(super::menu::add_overlay_builder(
                root,
                child_id,
                child_index,
                child_layer,
            ))
        } else {
            root.context.state_mut(root_id).set_custom_data(RootState {
                is_open: 0,
                trigger_width_bits,
            });
            None
        };

        if let Some(overlay) = overlay.as_mut() {
            overlay_hovered = super::menu::prepare_overlay(
                overlay,
                StyleClass::DropdownMenu,
                trigger_width.map(Size::Fixed),
            );
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
        T: DropdownItem,
    {
        self.item_inner(&callback)
    }

    pub fn finish(self) -> (Option<usize>, Interaction) {
        self.popup.finish(|state| {
            if let Some(root) = state.custom_data_mut::<RootState>() {
                root.is_open = 0;
            }
        })
    }

    fn item_inner(&mut self, callback: &dyn DropdownItem) -> &mut Self {
        self.popup
            .item(StyleClass::DropdownItem, Some(StyleClass::Button), callback);
        self
    }
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct RootState {
    is_open: u32,
    trigger_width_bits: u32,
}

unsafe impl bytemuck::Pod for RootState {}
unsafe impl bytemuck::Zeroable for RootState {}
