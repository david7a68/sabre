#[derive(Clone, Debug, Default)]
pub struct WindowSize {
    pub width: f32,
    pub height: f32,
}

#[derive(Clone, Debug, Default)]
pub struct PointerLocation {
    pub x: f32,
    pub y: f32,
}

#[derive(Clone, Debug, Default)]
pub struct MouseButtonState {
    pub is_left_down: bool,
    pub is_right_down: bool,
    pub is_middle_down: bool,
}

#[derive(Clone, Debug, Default)]
pub struct InputState {
    pub pointer: PointerLocation,
    pub mouse_state: MouseButtonState,
    pub window_size: WindowSize,
}
