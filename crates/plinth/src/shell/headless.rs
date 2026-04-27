//! Headless test driver for the UI layer.
//!
//! [`HeadlessRunner`] runs the same per-frame pipeline as the live winit shell
//! ([`run_ui_frame_core`]) but skips graphics submission. Each call to
//! [`HeadlessRunner::next_frame`] returns a [`FrameSnapshot`] of widget state
//! and layout for assertion.
//!
//! [`UiContext`]: crate::ui::context::UiContext

use std::time::Duration;

use crate::shell::Input;
use crate::ui::Theme;
use crate::ui::UiBuilder;
use crate::ui::context::FrameSnapshot;
use crate::ui::context::UiContext;

use super::WindowConfig;
use super::app_context::AppContext;
use super::app_context::run_ui_frame_core;

pub struct HeadlessRunner {
    runtime: AppContext,
    config: WindowConfig,
    input: Input,
    ui_context: UiContext,
}

impl HeadlessRunner {
    pub fn new(config: WindowConfig) -> Self {
        Self::with_theme(config, Theme::default())
    }

    pub fn with_theme(config: WindowConfig, theme: Theme) -> Self {
        let runtime = AppContext::new(theme);

        let mut input = Input::default();
        input.window_size.width = config.width as f32;
        input.window_size.height = config.height as f32;

        Self {
            runtime,
            config,
            input,
            ui_context: UiContext::default(),
        }
    }

    pub fn theme(&self) -> &Theme {
        self.runtime.theme()
    }

    pub fn theme_mut(&mut self) -> &mut Theme {
        self.runtime.theme_mut()
    }

    pub fn input(&self) -> &Input {
        &self.input
    }

    pub fn input_mut(&mut self) -> &mut Input {
        &mut self.input
    }

    /// Replaces the entire `Input` for the next frame. All transient state
    /// (mouse buttons, keyboard events, modifiers) is overwritten.
    pub fn replace_input(&mut self, input: Input) {
        self.input = input;
    }

    pub fn resize_viewport(&mut self, width: u32, height: u32) {
        self.config.width = width;
        self.config.height = height;
        self.input.window_size.width = width as f32;
        self.input.window_size.height = height as f32;
    }

    pub fn focus_changed(&mut self) {
        self.input.focus_changed();
    }

    /// Runs one frame with the current viewport input and a zero time delta.
    pub fn next_frame(&mut self, run_ui: impl FnOnce(UiBuilder)) -> FrameSnapshot {
        self.next_frame_with_delta(Duration::ZERO, run_ui)
    }

    /// Replaces input, then runs one frame. Equivalent to `replace_input` +
    /// `next_frame`. Note: this clobbers all input state, including keyboard
    /// events and mouse button counters.
    pub fn next_frame_with_input(
        &mut self,
        input: Input,
        run_ui: impl FnOnce(UiBuilder),
    ) -> FrameSnapshot {
        self.replace_input(input);
        self.next_frame(run_ui)
    }

    pub fn next_frame_with_delta(
        &mut self,
        time_delta: Duration,
        run_ui: impl FnOnce(UiBuilder),
    ) -> FrameSnapshot {
        let AppContext {
            clipboard,
            deferred_commands,
            theme,
            graphics: _,
            text_system,
            text_layouts,
            format_buffer,
        } = &mut self.runtime;

        run_ui_frame_core(
            &mut self.input,
            &mut self.ui_context,
            deferred_commands,
            clipboard,
            text_system,
            text_layouts,
            format_buffer,
            theme,
            time_delta,
            |_deferred_commands, ui_builder| {
                run_ui(ui_builder);
            },
        );

        self.ui_context.finish_layout(text_system, text_layouts);

        self.snapshot()
    }

    /// Snapshot of the most recently completed frame.
    pub fn snapshot(&self) -> FrameSnapshot {
        self.ui_context.snapshot()
    }
}

#[cfg(test)]
mod tests {
    use glamour::Point2;
    use winit::keyboard::KeyCode;
    use winit::keyboard::PhysicalKey;

    use crate::shell::ElementState;
    use crate::shell::KeyboardEvent;
    use crate::shell::WindowConfig;
    use crate::ui::AxisAnchor;
    use crate::ui::CommonWidgetsExt;
    use crate::ui::OverlayPosition;
    use crate::ui::WidgetId;

    use super::HeadlessRunner;

    fn config() -> WindowConfig {
        WindowConfig {
            title: "Headless Test".into(),
            width: 640,
            height: 480,
        }
    }

    fn custom_u32(bytes: [u8; 8]) -> u32 {
        u32::from_ne_bytes(bytes[..4].try_into().unwrap())
    }

    fn key_event(code: KeyCode, state: ElementState) -> KeyboardEvent {
        KeyboardEvent {
            key: PhysicalKey::Code(code),
            text: None,
            location: keyboard_types::Location::Standard,
            is_repeat: false,
            state,
        }
    }

    #[test]
    fn custom_data_persists_with_stable_widget_id() {
        let mut runner = HeadlessRunner::new(config());
        let expected_id = WidgetId::new("root").then("counter");

        let frame1 = runner.next_frame(|mut ui| {
            let mut counter = ui.named_child("counter");
            counter.size(120.0, 28.0);

            let value = counter.custom_data::<u32>().unwrap_or(0) + 1;
            counter.set_custom_data(value);
        });

        let widget1 = frame1.widgets.get(&expected_id).unwrap();

        assert!(widget1.has_custom_data);
        assert_eq!(custom_u32(widget1.custom_data), 1);
        assert_eq!(frame1.frame_counter, 1);

        let frame2 = runner.next_frame(|mut ui| {
            let mut counter = ui.named_child("counter");
            counter.size(120.0, 28.0);

            let value = counter.custom_data::<u32>().unwrap_or(0) + 1;
            counter.set_custom_data(value);
        });

        let widget2 = frame2.widgets.get(&expected_id).unwrap();

        assert!(widget2.has_custom_data);
        assert_eq!(custom_u32(widget2.custom_data), 2);
        assert_eq!(frame2.frame_counter, 2);
    }

    #[test]
    fn widget_state_cleans_up_when_not_rendered() {
        let mut runner = HeadlessRunner::new(config());

        let kept_id = WidgetId::new("root").then("kept");
        let removed_id = WidgetId::new("root").then("removed");

        runner.next_frame(|mut ui| {
            let mut kept = ui.named_child("kept");
            kept.size(80.0, 20.0);
            kept.set_custom_data(11u32);

            let mut removed = ui.named_child("removed");
            removed.size(80.0, 20.0);
            removed.set_custom_data(77u32);
        });

        let frame2 = runner.next_frame(|mut ui| {
            let mut kept = ui.named_child("kept");
            kept.size(80.0, 20.0);
            kept.set_custom_data(12u32);
        });

        assert!(frame2.widgets.contains_key(&kept_id));
        assert!(!frame2.widgets.contains_key(&removed_id));
    }

    #[test]
    fn simulated_pointer_press_activates_button() {
        let mut runner = HeadlessRunner::new(config());
        let button_id = WidgetId::new("root").then("Press");

        let frame1 = runner.next_frame(|mut ui| {
            let _ = ui.text_button("Press");
        });

        let placement = frame1.widgets.get(&button_id).unwrap().placement;

        runner.input_mut().pointer = Point2 {
            x: placement.origin.x + placement.size.width * 0.5,
            y: placement.origin.y + placement.size.height * 0.5,
        };
        runner.input_mut().mouse_state.left_click_count = 1;

        let mut activated = false;
        let _ = runner.next_frame(|mut ui| {
            let interaction = ui.text_button("Press");
            activated = interaction.is_activated;
        });

        assert!(activated);
    }

    #[test]
    fn layout_snapshot_preserves_layer_order() {
        let mut runner = HeadlessRunner::new(config());

        let snapshot = runner.next_frame(|mut ui| {
            let mut base = ui.named_child("base");
            base.size(200.0, 60.0);

            let mut overlay = base.overlay_child(
                "overlay",
                OverlayPosition {
                    parent_x: AxisAnchor::Start,
                    parent_y: AxisAnchor::End,
                    self_x: AxisAnchor::Start,
                    self_y: AxisAnchor::Start,
                    offset: (0.0, 0.0),
                    flip_x: false,
                    flip_y: true,
                },
            );
            overlay.size(150.0, 40.0);
        });

        assert!(!snapshot.layout_nodes.is_empty());

        let mut last_layer = 0u8;
        for node in snapshot.layout_nodes.iter() {
            assert!(node.z_layer >= last_layer);
            last_layer = node.z_layer;
        }

        assert!(snapshot.layout_nodes.iter().any(|node| node.z_layer == 0));
        assert!(snapshot.layout_nodes.iter().any(|node| node.z_layer > 0));
    }

    /// A modal overlay should set `input_block_layer` so lower-layer widgets
    /// cannot be hovered, even when the pointer sits over them.
    #[test]
    fn modal_overlay_blocks_lower_layer_hover() {
        let mut runner = HeadlessRunner::new(config());
        let base_id = WidgetId::new("root").then("base");

        // Frame 1: place a base widget and a modal overlay above it.
        let frame1 = runner.next_frame(|mut ui| {
            let mut base = ui.named_child("base");
            base.size(200.0, 60.0);

            let mut modal = base.modal_child(
                "modal",
                OverlayPosition {
                    parent_x: AxisAnchor::Start,
                    parent_y: AxisAnchor::Start,
                    self_x: AxisAnchor::Start,
                    self_y: AxisAnchor::Start,
                    offset: (0.0, 0.0),
                    flip_x: false,
                    flip_y: false,
                },
            );
            modal.size(80.0, 30.0);
        });

        let base_placement = frame1.widgets.get(&base_id).unwrap().placement;

        // Park the pointer over the base widget but outside the modal.
        runner.input_mut().pointer = Point2 {
            x: base_placement.origin.x + base_placement.size.width - 1.0,
            y: base_placement.origin.y + base_placement.size.height - 1.0,
        };

        let frame2 = runner.next_frame(|mut ui| {
            let mut base = ui.named_child("base");
            base.size(200.0, 60.0);

            let mut modal = base.modal_child(
                "modal",
                OverlayPosition {
                    parent_x: AxisAnchor::Start,
                    parent_y: AxisAnchor::Start,
                    self_x: AxisAnchor::Start,
                    self_y: AxisAnchor::Start,
                    offset: (0.0, 0.0),
                    flip_x: false,
                    flip_y: false,
                },
            );
            modal.size(80.0, 30.0);
        });

        // The modal should have registered as the current input block layer.
        assert!(
            frame2.input_block_layer.is_some(),
            "expected a modal overlay to set input_block_layer, got None",
        );
        let block_layer = frame2.input_block_layer.unwrap();
        let base_layer = frame2.widgets.get(&base_id).unwrap().layer;
        assert!(
            base_layer < block_layer,
            "base widget layer ({base_layer}) should be strictly below block layer ({block_layer})",
        );
    }

    /// Keyboard events are per-frame: `keyboard_events` must be empty after
    /// a frame consumes them, regardless of what the UI did with them.
    #[test]
    fn keyboard_events_clear_between_frames() {
        let mut runner = HeadlessRunner::new(config());

        runner
            .input_mut()
            .keyboard_events
            .push(key_event(KeyCode::Enter, ElementState::Pressed));
        assert_eq!(runner.input().keyboard_events.len(), 1);

        runner.next_frame(|mut ui| {
            let mut node = ui.named_child("node");
            node.size(10.0, 10.0);
        });

        assert!(
            runner.input().keyboard_events.is_empty(),
            "keyboard_events should be drained after the frame",
        );

        // A second frame with no input should remain empty.
        runner.next_frame(|mut ui| {
            let mut node = ui.named_child("node");
            node.size(10.0, 10.0);
        });
        assert!(runner.input().keyboard_events.is_empty());
    }

    /// `prev_pointer` is updated to `pointer` at end of frame, so a stationary
    /// pointer reads `prev_pointer == pointer` on the following frame.
    #[test]
    fn prev_pointer_tracks_pointer_across_frames() {
        let mut runner = HeadlessRunner::new(config());

        runner.input_mut().pointer = Point2 { x: 10.0, y: 20.0 };
        runner.next_frame(|mut ui| {
            let mut node = ui.named_child("node");
            node.size(10.0, 10.0);
        });

        assert_eq!(runner.input().prev_pointer, Point2 { x: 10.0, y: 20.0 });

        runner.input_mut().pointer = Point2 { x: 30.0, y: 40.0 };
        runner.next_frame(|mut ui| {
            let mut node = ui.named_child("node");
            node.size(10.0, 10.0);
        });

        assert_eq!(runner.input().prev_pointer, Point2 { x: 30.0, y: 40.0 });
    }
}
