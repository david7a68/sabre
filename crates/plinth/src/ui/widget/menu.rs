use std::hash::Hash;

use crate::ui::OverlayPosition;
use crate::ui::Position;
use crate::ui::TextOverflow;
use crate::ui::UiElementId;
use crate::ui::WidgetId;
use winit::keyboard::KeyCode;
use winit::keyboard::PhysicalKey;

use crate::ui::Size;
use crate::ui::StyleClass;
use crate::ui::UiBuilder;
use crate::ui::style::StateFlags;
use crate::ui::widget::PointerButton;
use crate::ui::widget::WidgetState;

use super::ClickBehavior;
use super::Interaction;

pub(super) struct MenuPopup<'a> {
    builder: Option<UiBuilder<'a>>,
    root_id: WidgetId,
    interaction: Interaction,
    menu: MenuList,
}

impl<'a> MenuPopup<'a> {
    pub fn new(
        overlay: Option<UiBuilder<'a>>,
        root_id: WidgetId,
        interaction: Interaction,
        close_requested: bool,
    ) -> Self {
        let prev_overlay_state = overlay
            .as_ref()
            .and_then(|o| o.prev_state())
            .and_then(|s| s.custom_data::<MenuOverlayState>())
            .unwrap_or_default();
        let mut menu = MenuList::from_overlay_state(prev_overlay_state);
        menu.close_requested = close_requested;

        Self {
            builder: overlay,
            root_id,
            interaction,
            menu,
        }
    }

    pub fn width(&mut self, width: impl Into<Size>) {
        if let Some(builder) = self.builder.as_mut() {
            builder.width(width);
        }
    }

    pub fn height(&mut self, height: impl Into<Size>) {
        if let Some(builder) = self.builder.as_mut() {
            builder.height(height);
        }
    }

    pub fn size(&mut self, width: impl Into<Size>, height: impl Into<Size>) {
        if let Some(builder) = self.builder.as_mut() {
            builder.size(width, height);
        }
    }

    pub fn padding(&mut self, padding: crate::ui::Padding) {
        if let Some(builder) = self.builder.as_mut() {
            builder.padding(padding);
        }
    }

    pub fn item(
        &mut self,
        item_style: StyleClass,
        padding_style: Option<StyleClass>,
        callback: &dyn MenuItem,
    ) {
        let Some(builder) = self.builder.as_mut() else {
            return;
        };

        self.menu.item(builder, item_style, padding_style, callback);
    }

    pub fn finish(
        mut self,
        close_root: impl FnOnce(&mut WidgetState),
    ) -> (Option<usize>, Interaction) {
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
                close_root(builder.context.state_mut(self.root_id));
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
}

pub(super) struct DismissState {
    activated_buttons: u8,
}

impl DismissState {
    pub fn any_activated(&self) -> bool {
        self.activated_buttons != 0
    }

    pub fn button_activated(&self, button: PointerButton) -> bool {
        self.activated_buttons & button.bit() != 0
    }
}

pub(super) fn dismiss_state(
    root: &mut UiBuilder<'_>,
    id: impl Hash,
    buttons: &[PointerButton],
) -> DismissState {
    let window_w = root.input.window_size.width;
    let window_h = root.input.window_size.height;
    let mut dismiss =
        root.modal_offset_child((id, "dismiss"), Position::Absolute { x: 0.0, y: 0.0 }, 2);
    dismiss.size(window_w, window_h);

    let mut activated_buttons = 0;
    for &button in buttons {
        let (interaction, _) = Interaction::compute_for_button(
            &dismiss,
            button,
            ClickBehavior::OnPress,
            StateFlags::HOVERED | StateFlags::PRESSED,
        );
        if interaction.is_activated {
            activated_buttons |= button.bit();
        }

        let active = button.is_down(&dismiss.input().mouse_state);
        dismiss
            .context
            .state_mut(dismiss.id)
            .set_pointer_button_active(button, active);
    }

    DismissState { activated_buttons }
}

pub(super) fn add_overlay_builder<'a>(
    root: UiBuilder<'a>,
    child_id: WidgetId,
    child_index: UiElementId,
    child_layer: u8,
) -> UiBuilder<'a> {
    UiBuilder {
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
    }
}

pub(super) fn add_root_overlay(
    root: &mut UiBuilder<'_>,
    child_id: WidgetId,
    pos: OverlayPosition,
    child_layer: u8,
) -> UiElementId {
    let child_index = root.context.add_overlay_node(
        root.index,
        child_id,
        Position::OutOfFlow(pos),
        child_layer,
        true,
    );
    root.num_child_widgets += 1;
    child_index
}

pub(super) fn prepare_overlay(
    overlay: &mut UiBuilder<'_>,
    style: StyleClass,
    width: Option<Size>,
) -> bool {
    let (overlay_interaction, overlay_state) = Interaction::compute(
        overlay,
        ClickBehavior::OnPress,
        StateFlags::HOVERED | StateFlags::PRESSED,
    );

    overlay.apply_style(style, overlay_state);
    if let Some(width) = width {
        overlay.width(width);
    }
    overlay.child_spacing(0.0);
    overlay.set_active(overlay_state.contains(StateFlags::PRESSED));
    overlay.child_direction(crate::ui::LayoutDirection::Vertical);
    overlay.request_focus();

    overlay_interaction.is_hovered
}

#[derive(Default)]
pub(super) struct MenuList {
    pub num_items: u32,
    pub highlighted_index: Option<u32>,
    pub keyboard_active: bool,
    pub selected_index: Option<u32>,
    pub close_requested: bool,
}

impl MenuList {
    pub fn from_overlay_state(state: MenuOverlayState) -> Self {
        Self {
            highlighted_index: state.highlighted_index(),
            keyboard_active: state.keyboard_active != 0,
            ..Default::default()
        }
    }

    pub fn overlay_state(&self) -> MenuOverlayState {
        MenuOverlayState {
            highlighted: self
                .highlighted_index
                .filter(|idx| *idx < self.num_items)
                .unwrap_or(MenuOverlayState::NO_HIGHLIGHT),
            keyboard_active: self.keyboard_active as u32,
        }
    }

    pub fn selected_index(&self) -> Option<usize> {
        self.selected_index.map(|i| i as usize)
    }

    pub fn item(
        &mut self,
        builder: &mut UiBuilder<'_>,
        item_style: StyleClass,
        padding_style: Option<StyleClass>,
        callback: &dyn MenuItem,
    ) {
        let item_index = self.num_items;
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

        let padding =
            padding_style.map(|style| item.theme().get(style).padding.get(effective_state));

        item.apply_style(item_style, effective_state);
        item.set_clip_children(true);
        item.set_active(item_state.contains(StateFlags::PRESSED));
        if let Some(padding) = padding {
            item.padding(padding);
        }

        callback.build(&mut item);

        self.num_items += 1;
    }

    pub fn handle_keyboard_input(&mut self, builder: &UiBuilder<'_>) {
        if !builder.is_focused() {
            return;
        }

        if self
            .highlighted_index
            .is_some_and(|idx| idx >= self.num_items)
        {
            self.highlighted_index = None;
        }

        for event in builder.input().keyboard_events.iter() {
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

pub trait MenuItem {
    fn build(&self, builder: &mut UiBuilder);
}

impl MenuItem for &dyn MenuItem {
    fn build(&self, builder: &mut UiBuilder) {
        (**self).build(builder);
    }
}

impl MenuItem for &str {
    fn build(&self, builder: &mut UiBuilder) {
        builder.text(self, Size::default());
    }
}

impl MenuItem for String {
    fn build(&self, builder: &mut UiBuilder) {
        builder.text(self.as_str(), Size::default());
    }
}

impl<F> MenuItem for F
where
    F: Fn(&mut UiBuilder),
{
    fn build(&self, builder: &mut UiBuilder) {
        self(builder);
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub(super) struct MenuOverlayState {
    highlighted: u32,
    keyboard_active: u32,
}

impl MenuOverlayState {
    const NO_HIGHLIGHT: u32 = u32::MAX;

    fn highlighted_index(self) -> Option<u32> {
        (self.highlighted != Self::NO_HIGHLIGHT).then_some(self.highlighted)
    }
}

impl Default for MenuOverlayState {
    fn default() -> Self {
        Self {
            highlighted: Self::NO_HIGHLIGHT,
            keyboard_active: 0,
        }
    }
}

unsafe impl bytemuck::Pod for MenuOverlayState {}
unsafe impl bytemuck::Zeroable for MenuOverlayState {}
