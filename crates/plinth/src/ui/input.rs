use glamour::Point2;
use keyboard_types::Location;
use smallvec::SmallVec;
use winit::keyboard::PhysicalKey;
use winit::keyboard::SmolStr;

use crate::ui::Pixels;

#[derive(Clone, Copy, Debug, Default)]
pub struct WindowSize {
    pub width: f32,
    pub height: f32,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct MouseButtonState {
    pub left_click_count: u8,
    pub right_click_count: u8,
    pub middle_click_count: u8,
}

impl MouseButtonState {
    pub fn is_left_down(&self) -> bool {
        self.left_click_count > 0
    }

    pub fn is_right_down(&self) -> bool {
        self.right_click_count > 0
    }

    pub fn is_middle_down(&self) -> bool {
        self.middle_click_count > 0
    }
}

#[derive(Clone, Debug, Default)]
pub struct Input {
    pub pointer: Point2<Pixels>,
    pub mouse_state: MouseButtonState,
    pub window_size: WindowSize,
    pub keyboard_events: SmallVec<[KeyboardEvent; 4]>,
    pub modifiers: winit::keyboard::ModifiersState,
}

#[derive(Clone, Debug)]
pub struct KeyboardEvent {
    pub key: PhysicalKey,
    pub text: Option<SmolStr>,
    pub location: Location,
    pub is_repeat: bool,
    pub state: ElementState,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ElementState {
    Pressed,
    Released,
}

impl ElementState {
    pub fn is_pressed(&self) -> bool {
        matches!(self, ElementState::Pressed)
    }

    pub fn is_released(&self) -> bool {
        matches!(self, ElementState::Released)
    }
}

impl From<winit::event::ElementState> for ElementState {
    fn from(value: winit::event::ElementState) -> Self {
        match value {
            winit::event::ElementState::Pressed => Self::Pressed,
            winit::event::ElementState::Released => Self::Released,
        }
    }
}
