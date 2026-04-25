use std::time::Duration;
use std::time::Instant;

use glamour::Contains;
use glamour::Point2;
use glamour::Size2;
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
    pub prev_pointer: Point2<Pixels>,
    pub mouse_state: MouseButtonState,
    pub window_size: WindowSize,
    pub keyboard_events: SmallVec<[KeyboardEvent; 4]>,
    pub modifiers: winit::keyboard::ModifiersState,
}

impl Input {
    pub fn focus_changed(&mut self) {
        *self = Self {
            window_size: self.window_size,
            ..Default::default()
        }
    }
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

// Windows default value, good enough if we can't get the system settings.
const DEFAULT_MAX_CLICK_INTERVAL: Duration = Duration::from_millis(500);
const DEFAULT_MAX_CLICK_SLOP: f32 = 4.0;
const WINDOWS_STANDARD_DPI: f64 = 96.0;

/// Tracks double-click state for mouse buttons.
///
/// Winit doesn't currently provide double-click events, so we have to track
/// them ourselves.
pub(crate) struct DoubleClickTracker {
    last_click_time: Instant,
    last_click_button: winit::event::MouseButton,
    last_click_position: glamour::Point2<Pixels>,
    last_click_count: u8,

    max_click_interval: Duration,
    max_click_slop: glamour::Size2<Pixels>,
}

impl DoubleClickTracker {
    pub fn load_parameters(scale_factor: f64) -> Self {
        let max_click_interval;
        let max_click_slop;

        #[cfg(target_os = "windows")]
        {
            use windows_sys::Win32::UI::HiDpi::GetSystemMetricsForDpi;
            use windows_sys::Win32::UI::Input::KeyboardAndMouse::GetDoubleClickTime;
            use windows_sys::Win32::UI::WindowsAndMessaging::SM_CXDOUBLECLK;
            use windows_sys::Win32::UI::WindowsAndMessaging::SM_CYDOUBLECLK;

            let dpi = (scale_factor * WINDOWS_STANDARD_DPI).round() as u32;

            let get_slop = |metric| {
                let value = unsafe { GetSystemMetricsForDpi(metric, dpi) };
                if value == 0 {
                    DEFAULT_MAX_CLICK_SLOP * scale_factor as f32
                } else {
                    value as f32
                }
            };

            max_click_slop = Size2::new(get_slop(SM_CXDOUBLECLK), get_slop(SM_CYDOUBLECLK));

            let interval = unsafe { GetDoubleClickTime() };
            if interval == 0 {
                max_click_interval = DEFAULT_MAX_CLICK_INTERVAL;
            } else {
                max_click_interval = Duration::from_millis(interval as u64);
            }
        }

        #[cfg(not(target_os = "windows"))]
        {
            max_click_interval = DEFAULT_MAX_CLICK_INTERVAL;
            max_click_slop = Size2::new(
                DEFAULT_MAX_CLICK_SLOP * scale_factor as f32,
                DEFAULT_MAX_CLICK_SLOP * scale_factor as f32,
            );
        }

        Self {
            last_click_time: Instant::now(),
            last_click_button: winit::event::MouseButton::Left,
            last_click_position: Point2::new(0.0, 0.0),
            last_click_count: 0,
            max_click_interval,
            max_click_slop,
        }
    }

    pub fn on_dpi_changed(&mut self, dpi: f64) {
        *self = Self::load_parameters(dpi);
    }

    pub fn on_click(
        &mut self,
        button: winit::event::MouseButton,
        state: winit::event::ElementState,
        position: Point2<Pixels>,
    ) -> u8 {
        if state == winit::event::ElementState::Released {
            return 0;
        }

        let time = Instant::now();

        let last_rect = glamour::Rect::new(self.last_click_position, Size2::new(0.0, 0.0))
            .inflate(self.max_click_slop);

        let contains_point = last_rect.contains(&position);
        let time_delta = time.duration_since(self.last_click_time);

        if !contains_point
            || time_delta > self.max_click_interval
            || button != self.last_click_button
        {
            self.last_click_count = 0;
        }

        self.last_click_button = button;
        self.last_click_count += 1;
        self.last_click_position = position;
        self.last_click_time = time;

        self.last_click_count
    }

    pub fn on_activate(&mut self) {
        self.last_click_count = 0;
    }
}
