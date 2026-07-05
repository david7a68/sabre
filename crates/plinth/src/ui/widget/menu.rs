use winit::keyboard::KeyCode;
use winit::keyboard::PhysicalKey;

use crate::ui::Size;
use crate::ui::StyleClass;
use crate::ui::UiBuilder;
use crate::ui::style::StateFlags;

use super::ClickBehavior;
use super::Interaction;

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
