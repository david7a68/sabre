use std::time::Duration;
use std::time::Instant;

use glamour::Contains;
use glamour::Point2;
use glamour::Size2;

use crate::ui::Pixels;

/// Tracks double-click state for mouse buttons.
///
/// Winit doesn't currently provide double-click events, so we have to track
/// them ourselves.
pub(super) struct DoubleClickTracker {
    last_click_time: Instant,
    last_click_button: winit::event::MouseButton,
    last_click_position: glamour::Point2<Pixels>,
    last_click_count: u8,

    max_click_interval: Duration,
    max_click_slop: glamour::Size2<Pixels>,
}

impl DoubleClickTracker {
    pub fn load_parameters(dpi: f64) -> Self {
        let max_click_interval;
        let max_click_slop;

        #[cfg(target_os = "windows")]
        {
            use windows_sys::Win32::UI::HiDpi::GetSystemMetricsForDpi;
            use windows_sys::Win32::UI::Input::KeyboardAndMouse::GetDoubleClickTime;
            use windows_sys::Win32::UI::WindowsAndMessaging::SM_CXDOUBLECLK;
            use windows_sys::Win32::UI::WindowsAndMessaging::SM_CYDOUBLECLK;

            let dpi = dpi.round() as u32;
            let max_click_slop_x = unsafe { GetSystemMetricsForDpi(SM_CXDOUBLECLK, dpi) } as f32;
            let max_click_slop_y = unsafe { GetSystemMetricsForDpi(SM_CYDOUBLECLK, dpi) } as f32;

            max_click_slop = Size2::new(max_click_slop_x, max_click_slop_y);
            max_click_interval = Duration::from_millis(unsafe { GetDoubleClickTime() as u64 });
        }

        #[cfg(not(target_os = "windows"))]
        {
            // stuff here
            max_click_interval = Duration::from_millis(500);
            max_click_slop = Size2::new(4.0 * dpi, 4.0 * dpi);
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
