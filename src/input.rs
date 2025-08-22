use glamour::Point2;

use crate::Pixels;

#[derive(Clone, Copy, Debug, Default)]
pub struct WindowSize {
    pub width: f32,
    pub height: f32,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct MouseButtonState {
    pub is_left_down: bool,
    pub is_right_down: bool,
    pub is_middle_down: bool,
}

#[derive(Clone, Debug, Default)]
pub struct InputState {
    pub pointer: Point2<Pixels>,
    pub mouse_state: MouseButtonState,
    pub window_size: WindowSize,
}
